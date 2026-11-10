//! Small plain-JSON stores next to the vault: the app settings, the currency
//! cache, the recent vaults. One reader and one writer for all of them.
//!
//! Each of them used to read itself with `read_to_string().ok().and_then(…)`,
//! which folds "no file yet" and "the file is there but unreadable" into the
//! same `default()`: a user whose settings file got truncated silently
//! started over as if the app had just been installed. [`read`] keeps the two
//! apart, and keeps the bytes of a broken file instead of letting the next
//! save overwrite them.
//!
//! Writes go through the vault's atomic path (temp file + fsync + rename), so
//! a write cut short by a crash or a full disk cannot leave a half-written
//! store that the next start reads as a first run.
//!
//! Nothing here is secret: a store that needs encryption belongs in the vault
//! or in `crypto::device_secrets`, not in this module.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::vault_io::local::atomic_write;

/// Suffix a store that would not parse is kept under.
const BROKEN_SUFFIX: &str = ".broken";

/// What reading a small JSON store found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load<T> {
    /// No file yet: a first run, and the only case that means it.
    Missing,
    /// The stored value.
    Ok(T),
    /// Unreadable: the default to use, where the bad file was kept, and why.
    Broken {
        value: T,
        kept: Option<PathBuf>,
        error: String,
    },
}

impl<T: Default> Load<T> {
    /// The value to work with, reporting a broken store once under `label`.
    ///
    /// Every caller starts on defaults either way — a settings file must
    /// never keep the app from opening — so the only thing that changes with
    /// a broken store is the line in the log and the kept copy.
    pub fn or_default_logged(self, label: &str) -> T {
        match self {
            Load::Missing => T::default(),
            Load::Ok(value) => value,
            Load::Broken { value, kept, error } => {
                match kept {
                    Some(kept) => eprintln!(
                        "{label}: unreadable, starting from defaults ({error}); the file is kept as {}",
                        kept.display()
                    ),
                    None => {
                        eprintln!("{label}: unreadable, starting from defaults ({error})");
                    }
                }
                value
            }
        }
    }
}

/// Read a small JSON store, telling a first run from an unreadable file.
///
/// A file that does not parse is moved aside as `<name>.broken` so its bytes
/// survive the next save. An existing `.broken` is never overwritten (the
/// same rule as the `.vN.bak` of a migrated vault): the first failure is the
/// one worth keeping.
pub fn read<T: DeserializeOwned + Default>(path: &Path) -> Load<T> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Load::Missing,
        // There is a file, we just cannot read it: not a first run, and
        // moving it aside would need the very access we lack.
        Err(err) => {
            return Load::Broken {
                value: T::default(),
                kept: None,
                error: err.to_string(),
            };
        }
    };
    match serde_json::from_str::<T>(&raw) {
        Ok(value) => Load::Ok(value),
        Err(err) => Load::Broken {
            value: T::default(),
            kept: keep_aside(path),
            error: err.to_string(),
        },
    }
}

/// Write a small JSON store atomically, creating its directory when needed.
pub fn write<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let body =
        serde_json::to_string_pretty(value).map_err(|err| Error::Serialize(err.to_string()))?;
    atomic_write(path, body.as_bytes())
        .map_err(|err| Error::Io(format!("cannot write {}: {err}", path.display())))
}

/// Where a broken store is kept: `app_settings.json.broken`.
pub fn broken_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(BROKEN_SUFFIX);
    path.with_file_name(name)
}

/// Move the unreadable file aside, keeping an older copy if there is one.
fn keep_aside(path: &Path) -> Option<PathBuf> {
    let target = broken_path(path);
    if target.exists() {
        // The first failure is already kept; this file stays where it is
        // until the next save replaces it.
        return Some(target);
    }
    std::fs::rename(path, &target).ok().map(|()| target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Prefs {
        theme: String,
        count: u32,
    }

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn a_store_that_was_never_written_is_missing_not_broken() {
        let dir = dir();
        let path = dir.path().join("prefs.json");
        let load = read::<Prefs>(&path);
        assert_eq!(load, Load::Missing);
        assert_eq!(load.or_default_logged("prefs"), Prefs::default());
        assert!(!broken_path(&path).exists());
    }

    #[test]
    fn a_written_store_reads_back_equal() {
        let dir = dir();
        let path = dir.path().join("nested").join("prefs.json");
        let prefs = Prefs {
            theme: "dark".into(),
            count: 7,
        };
        write(&path, &prefs).expect("write");
        assert_eq!(read::<Prefs>(&path), Load::Ok(prefs));
    }

    #[test]
    fn an_unreadable_store_is_kept_aside_and_reports_why() {
        let dir = dir();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, "{ not json").expect("write");

        let Load::Broken { value, kept, error } = read::<Prefs>(&path) else {
            panic!("a file that does not parse must not read as Ok or Missing");
        };
        assert_eq!(value, Prefs::default());
        assert_eq!(kept.as_deref(), Some(broken_path(&path).as_path()));
        assert!(!error.is_empty());
        assert!(!path.exists(), "the bad file was moved aside");
        assert_eq!(
            std::fs::read_to_string(broken_path(&path)).expect("kept copy"),
            "{ not json"
        );
    }

    #[test]
    fn a_later_broken_store_never_overwrites_the_first_one() {
        let dir = dir();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, "{ the first failure").expect("write");
        assert!(matches!(read::<Prefs>(&path), Load::Broken { .. }));

        // A later start finds another broken file: the first one stays.
        std::fs::write(&path, "{ a later failure").expect("write");
        let load = read::<Prefs>(&path);
        assert!(matches!(load, Load::Broken { .. }));
        assert_eq!(
            std::fs::read_to_string(broken_path(&path)).expect("kept copy"),
            "{ the first failure"
        );
        // Still readable as broken, and a save replaces it.
        write(&path, &Prefs::default()).expect("write");
        assert_eq!(read::<Prefs>(&path), Load::Ok(Prefs::default()));
    }

    #[test]
    fn a_temp_file_left_by_an_interrupted_write_leaves_the_store_readable() {
        let dir = dir();
        let path = dir.path().join("prefs.json");
        let prefs = Prefs {
            theme: "light".into(),
            count: 1,
        };
        write(&path, &prefs).expect("write");
        // What a crash between the temp file and the rename leaves behind.
        std::fs::write(dir.path().join(".prefs.json.tmp.1.deadbeef"), "half a file")
            .expect("write");

        assert_eq!(read::<Prefs>(&path), Load::Ok(prefs));
        // And a write still goes through, with a name of its own.
        write(&path, &Prefs::default()).expect("write");
        assert_eq!(read::<Prefs>(&path), Load::Ok(Prefs::default()));
    }
}
