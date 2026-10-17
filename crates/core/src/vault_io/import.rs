//! Copying a picked vault into the app's own storage.
//!
//! On Android and iOS the picked document cannot be written atomically, so
//! the vault the app works on is a copy in its data directory. The copy keeps
//! the user's file name; a different vault under the same name gets ` (2)`.

use std::path::{Path, PathBuf};

use crate::crypto;
use crate::error::{Error, Result};
use crate::storage::SnapshotFormat;
use crate::vault_io::local::{atomic_write, ensure_wallet_ext};

/// Check `bytes` look like a vault (magic, container version, payload
/// format) without the password, so a wrong file is refused before it is
/// copied anywhere.
pub fn validate_vault_bytes(bytes: &[u8]) -> Result<SnapshotFormat> {
    let (header, _) = crypto::parse_file(bytes)?;
    header.payload_format().ok_or(Error::Unsupported)
}

/// Write `bytes` into `dir` as `name` (with `.wallet`), never over another
/// vault. Picking the same file twice returns the copy made the first time.
pub fn import_into(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    validate_vault_bytes(bytes)?;
    let base = ensure_wallet_ext(Path::new(&safe_file_name(name)));
    let stem = base
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "vault".into());
    let ext = base
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned())
        .unwrap_or_default();
    for attempt in 1..=999 {
        let file = if attempt == 1 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem} ({attempt}).{ext}")
        };
        let target = dir.join(file);
        match std::fs::read(&target) {
            Ok(existing) if existing == bytes => return Ok(target),
            Ok(_) => continue,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                atomic_write(&target, bytes)?;
                return Ok(target);
            }
            Err(err) => return Err(Error::Io(err.to_string())),
        }
    }
    Err(Error::AlreadyExists(dir.join(base).display().to_string()))
}

/// A name that stays one file inside `dir`: no separators, no `..`, not empty.
pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|char| match char {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other if other.is_control() => '_',
            other => other,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        "vault".into()
    } else {
        cleaned.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::vault::VaultDoc;
    use crate::storage::{SnapshotFormat, encode};

    fn sealed(password: &str) -> Vec<u8> {
        let plaintext = encode(&VaultDoc::default(), SnapshotFormat::Json).unwrap();
        let (header, _, _) = crypto::random_header(SnapshotFormat::Json);
        crypto::seal(&header, password.as_bytes(), &plaintext).unwrap()
    }

    #[test]
    fn a_foreign_file_is_refused_before_copying() {
        let dir = tempfile::tempdir().unwrap();
        assert!(validate_vault_bytes(b"hello").is_err());
        assert!(import_into(dir.path(), "x.wallet", b"not a vault").is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn import_keeps_the_name_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let first = sealed("a");
        let second = sealed("b");

        let one = import_into(dir.path(), "money", &first).unwrap();
        assert_eq!(one, dir.path().join("money.wallet"));
        // The same file again: the earlier copy, not a duplicate.
        assert_eq!(
            import_into(dir.path(), "money.wallet", &first).unwrap(),
            one
        );
        // Another vault with the same name gets a suffix.
        let two = import_into(dir.path(), "money.wallet", &second).unwrap();
        assert_eq!(two, dir.path().join("money (2).wallet"));
        assert_eq!(std::fs::read(&one).unwrap(), first);
        assert_eq!(std::fs::read(&two).unwrap(), second);
        // The copy opens with its password.
        assert!(crate::storage::VaultFile::open(&two, "b").is_ok());
    }

    #[test]
    fn names_cannot_escape_the_directory() {
        assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_file_name("a/b\\c:d"), "a_b_c_d");
        assert_eq!(safe_file_name(" .. "), "vault");
        assert_eq!(safe_file_name(""), "vault");
        assert_eq!(safe_file_name("мой кошелёк.wallet"), "мой кошелёк.wallet");
    }
}
