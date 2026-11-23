//! Backup copies of a vault file: the `.bak` slots that sit next to it
//! (`vault.wallet.bak`, `vault.wallet.v2.bak`, `vault.wallet.presync.bak`) and
//! the copies the user takes by hand (`vault.wallet.20261022-094000.bak`).
//!
//! The backup folder is the vault's own folder, and a backup is a file there
//! named `<vault file name>.<slot>.bak` (see [`is_backup_name`]). [`resolve`] is the only way from
//! a name the UI sends to a path: a name that points anywhere else is refused.

use std::path::{Path, PathBuf};

use crate::vault_io::local::atomic_write;
use mwt_base::{Error, Result};

/// A backup file as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupFile {
    /// File name inside the backup folder.
    pub name: String,
    /// RFC 3339 time the file was last written.
    pub modified_at: String,
    pub size_bytes: u64,
}

/// The folder backups live in: the one that holds the vault.
pub fn backup_dir(vault: &Path) -> PathBuf {
    match vault.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn vault_name(vault: &Path) -> Result<String> {
    vault
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| Error::InvalidInput("the vault has no file name".into()))
}

fn all_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// `20261022-094000` or `20261022-094000-2`.
fn is_stamp_slot(middle: &str) -> bool {
    let mut parts = middle.splitn(3, '-');
    let (Some(day), Some(time)) = (parts.next(), parts.next()) else {
        return false;
    };
    let attempt_ok = parts.next().is_none_or(all_digits);
    day.len() == 8 && all_digits(day) && time.len() == 6 && all_digits(time) && attempt_ok
}

/// Whether `file` is a backup of the vault called `vault_name`: the vault name,
/// a dot, then only what the app writes: `bak`, `vN.bak`, `presync[.N].bak`,
/// `<yyyymmdd-hhmmss>[-N].bak`. Another vault's copy (`family.wallet.old.wallet.….bak`)
/// has more after the dot and is not a match.
fn is_backup_name(vault_name: &str, file: &str) -> bool {
    let Some(tail) = file
        .strip_prefix(vault_name)
        .and_then(|rest| rest.strip_prefix('.'))
    else {
        return false;
    };
    if tail == "bak" {
        return true;
    }
    let Some(middle) = tail.strip_suffix(".bak") else {
        return false;
    };
    if let Some(version) = middle.strip_prefix('v') {
        return all_digits(version);
    }
    match middle.strip_prefix("presync") {
        Some("") => true,
        Some(index) => index.strip_prefix('.').is_some_and(all_digits),
        None => is_stamp_slot(middle),
    }
}

fn rfc3339(time: std::time::SystemTime) -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from(time)
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// The vault's backups, newest first. A folder that cannot be read is an error;
/// an unreadable single entry is skipped.
pub fn list(vault: &Path) -> Result<Vec<BackupFile>> {
    let name = vault_name(vault)?;
    let entries = std::fs::read_dir(backup_dir(vault)).map_err(|err| Error::Io(err.to_string()))?;
    let mut found: Vec<(std::time::SystemTime, BackupFile)> = Vec::new();
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        if !is_backup_name(&name, &file) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        // A link or a folder is not a copy the app wrote.
        if !meta.is_file() || entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
            continue;
        }
        let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        found.push((
            modified,
            BackupFile {
                name: file,
                modified_at: rfc3339(modified),
                size_bytes: meta.len(),
            },
        ));
    }
    found.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.name.cmp(&right.1.name))
    });
    Ok(found.into_iter().map(|(_, file)| file).collect())
}

/// The path of the backup called `name`, only if it really is one of this
/// vault's backups: a bare file name (no folders, no `..`), named after the
/// vault, ending in `.bak`, existing as a plain file in the backup folder.
pub fn resolve(vault: &Path, name: &str) -> Result<PathBuf> {
    let vault_file = vault_name(vault)?;
    let bare = Path::new(name)
        .file_name()
        .is_some_and(|only| only.to_string_lossy() == name);
    if !bare || name.contains(['/', '\\']) || !is_backup_name(&vault_file, name) {
        return Err(Error::InvalidInput(format!(
            "'{name}' is not a backup of this vault"
        )));
    }
    let path = backup_dir(vault).join(name);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| Error::NotFound(name.to_string()))?;
    if !meta.is_file() {
        return Err(Error::InvalidInput(format!(
            "'{name}' is not a backup file"
        )));
    }
    Ok(path)
}

/// Copy the vault file to a new, never-used backup name stamped with `stamp`
/// (`20261022-094000`) and report it.
pub fn create(vault: &Path, stamp: &str) -> Result<BackupFile> {
    let vault_file = vault_name(vault)?;
    let dir = backup_dir(vault);
    let mut target = dir.join(format!("{vault_file}.{stamp}.bak"));
    for attempt in 2.. {
        if !target.exists() {
            break;
        }
        target = dir.join(format!("{vault_file}.{stamp}-{attempt}.bak"));
    }
    let bytes = std::fs::read(vault).map_err(|err| Error::Io(err.to_string()))?;
    atomic_write(&target, &bytes)?;
    let name = target
        .file_name()
        .map(|only| only.to_string_lossy().into_owned())
        .unwrap_or_default();
    let meta = std::fs::metadata(&target).map_err(|err| Error::Io(err.to_string()))?;
    Ok(BackupFile {
        name,
        modified_at: rfc3339(meta.modified().unwrap_or(std::time::UNIX_EPOCH)),
        size_bytes: meta.len(),
    })
}

/// Delete this vault's backups that are not encrypted (made while it had no
/// password) and report how many went. Every file goes through [`resolve`]
/// (only this vault's backups in its own folder); an encrypted copy, a file
/// that is not a vault and a copy that cannot be read are kept.
pub fn remove_plain(vault: &Path) -> Result<u32> {
    let mut removed = 0;
    for file in list(vault)? {
        let path = resolve(vault, &file.name)?;
        if mwt_crypto::file_is_encrypted(&path) {
            continue;
        }
        std::fs::remove_file(&path).map_err(|err| Error::Io(err.to_string()))?;
        removed += 1;
    }
    Ok(removed)
}

/// `20261022-094000` for the moment `now`.
pub fn stamp(now: time::OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

/// Rotated pre-sync copies kept before the first batch of each session.
pub const PRESYNC_BACKUPS: usize = 3;

/// The pre-sync copy of `vault` at rotation `index` (0 is the newest):
/// `<vault>.presync.bak`, `<vault>.presync.<index>.bak`.
pub fn presync_path(vault: &Path, index: usize) -> PathBuf {
    let mut name = vault.file_name().unwrap_or_default().to_os_string();
    if index == 0 {
        name.push(".presync.bak");
    } else {
        name.push(format!(".presync.{index}.bak"));
    }
    vault.with_file_name(name)
}

/// Delete every pre-sync copy of `vault` (a missing file is fine).
pub fn remove_presync(vault: &Path) {
    for index in 0..PRESYNC_BACKUPS {
        let _ = std::fs::remove_file(presync_path(vault, index));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault.wallet");
        std::fs::write(&vault, b"vault").unwrap();
        (dir, vault)
    }

    #[test]
    fn lists_only_this_vaults_backups() {
        let (dir, vault) = setup();
        for file in [
            "vault.wallet.bak",
            "vault.wallet.v2.bak",
            "vault.wallet.20261022-094000.bak",
            "other.wallet.bak",
            "vault.wallet.lock",
            "vault.walletx.bak",
            "notes.txt",
        ] {
            std::fs::write(dir.path().join(file), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join("vault.wallet.v7.bak")).unwrap();
        let mut names: Vec<_> = list(&vault)
            .unwrap()
            .into_iter()
            .map(|file| file.name)
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "vault.wallet.20261022-094000.bak",
                "vault.wallet.bak",
                "vault.wallet.v2.bak"
            ]
        );
    }

    #[test]
    fn backup_names_accept_the_app_suffixes() {
        for name in [
            "vault.wallet.bak",
            "vault.wallet.v2.bak",
            "vault.wallet.v12.bak",
            "vault.wallet.presync.bak",
            "vault.wallet.presync.2.bak",
            "vault.wallet.20261022-094000.bak",
            "vault.wallet.20261022-094000-2.bak",
        ] {
            assert!(is_backup_name("vault.wallet", name), "{name} is a backup");
        }
    }

    #[test]
    fn backup_names_reject_everything_else() {
        for name in [
            "vault.wallet",
            "vault.walletx.bak",
            "vault.wallet.lock",
            "vault.wallet..bak",
            "vault.wallet.v.bak",
            "vault.wallet.vx.bak",
            "vault.wallet.junk.bak",
            "vault.wallet.presync.x.bak",
            "vault.wallet.2026-10-22.bak",
            "vault.wallet.20261022-094000-x.bak",
            "other.wallet.bak",
        ] {
            assert!(!is_backup_name("vault.wallet", name), "{name} is not");
        }
    }

    #[test]
    fn another_vaults_backup_is_not_mine() {
        assert!(!is_backup_name(
            "family.wallet",
            "family.wallet.old.wallet.20261022-094000.bak"
        ));
        assert!(is_backup_name(
            "family.wallet.old.wallet",
            "family.wallet.old.wallet.20261022-094000.bak"
        ));
    }

    #[test]
    fn a_copy_leaves_no_temp_file() {
        let (dir, vault) = setup();
        create(&vault, "20261022-094000").unwrap();
        let mut names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["vault.wallet", "vault.wallet.20261022-094000.bak"]);
    }

    #[test]
    fn create_never_overwrites_an_earlier_copy() {
        let (dir, vault) = setup();
        let first = create(&vault, "20261022-094000").unwrap();
        std::fs::write(&vault, b"changed").unwrap();
        let second = create(&vault, "20261022-094000").unwrap();
        assert_eq!(first.name, "vault.wallet.20261022-094000.bak");
        assert_eq!(second.name, "vault.wallet.20261022-094000-2.bak");
        assert_eq!(
            std::fs::read(dir.path().join(&first.name)).unwrap(),
            b"vault"
        );
        assert_eq!(
            std::fs::read(dir.path().join(&second.name)).unwrap(),
            b"changed"
        );
    }

    #[test]
    fn resolve_accepts_a_backup_in_the_folder() {
        let (dir, vault) = setup();
        std::fs::write(dir.path().join("vault.wallet.v2.bak"), b"x").unwrap();
        assert_eq!(
            resolve(&vault, "vault.wallet.v2.bak").unwrap(),
            dir.path().join("vault.wallet.v2.bak")
        );
    }

    #[test]
    fn resolve_refuses_a_path_outside_the_backup_folder() {
        let (dir, vault) = setup();
        let outside = tempfile::tempdir().unwrap();
        let elsewhere = outside.path().join("vault.wallet.v9.bak");
        std::fs::write(&elsewhere, b"x").unwrap();
        std::fs::write(dir.path().join("other.wallet.bak"), b"x").unwrap();
        std::fs::write(dir.path().join("vault.wallet.v9.bak"), b"x").unwrap();
        for name in [
            elsewhere.to_string_lossy().as_ref(),
            "../vault.wallet.v9.bak",
            "sub/vault.wallet.v9.bak",
            "sub\\vault.wallet.v9.bak",
            "./vault.wallet.v9.bak",
            "vault.wallet",
            "other.wallet.bak",
            "..",
            "",
        ] {
            assert!(
                matches!(resolve(&vault, name), Err(Error::InvalidInput(_))),
                "{name:?} must be refused"
            );
        }
        assert!(resolve(&vault, "vault.wallet.v9.bak").is_ok());
        assert!(matches!(
            resolve(&vault, "vault.wallet.v6.bak"),
            Err(Error::NotFound(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_refuses_a_link_out_of_the_folder() {
        let (dir, vault) = setup();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("secret");
        std::fs::write(&target, b"x").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("vault.wallet.v8.bak")).unwrap();
        assert!(resolve(&vault, "vault.wallet.v8.bak").is_err());
        assert!(list(&vault).unwrap().is_empty());
    }

    #[test]
    fn presync_copies_sit_next_to_the_vault_and_rotate_by_index() {
        let vault = Path::new("/tmp/home/v.wallet");
        assert_eq!(
            presync_path(vault, 0),
            Path::new("/tmp/home/v.wallet.presync.bak")
        );
        assert_eq!(
            presync_path(vault, 2),
            Path::new("/tmp/home/v.wallet.presync.2.bak")
        );
    }

    #[test]
    fn removing_presync_copies_leaves_the_vault_and_other_copies() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        std::fs::write(&vault, b"v").unwrap();
        let other = dir.path().join("v.wallet.bak");
        std::fs::write(&other, b"o").unwrap();
        for index in 0..PRESYNC_BACKUPS {
            std::fs::write(presync_path(&vault, index), b"p").unwrap();
        }
        remove_presync(&vault);
        for index in 0..PRESYNC_BACKUPS {
            assert!(!presync_path(&vault, index).exists());
        }
        assert!(vault.exists() && other.exists());
    }
}
