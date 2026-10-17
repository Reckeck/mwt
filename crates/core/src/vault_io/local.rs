//! The vault file on a plain file system: atomic replace, the instance lock
//! and the backup slots next to the file. Every platform keeps the *working*
//! vault on such a path (on Android and iOS inside the app's own storage), so
//! this is the only code that ever writes a vault.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Atomic save: write `bytes` to `path` via temp file + fsync + rename.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp_name = format!(
        ".{}.tmp.{}.{}",
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "vault".into()),
        std::process::id(),
        uuid::Uuid::new_v4().simple(),
    );
    let tmp_path = dir.join(tmp_name);

    let result = (|| -> Result<()> {
        let mut file = File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp_path, path)?;
        if let Ok(handle) = File::open(dir) {
            let _ = handle.sync_all(); // best-effort dir fsync
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

pub fn read_all(path: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            Error::NotFound(path.display().to_string())
        } else {
            Error::Io(err.to_string())
        }
    })?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Conventional backup slot for a vault file: `<name>.bak` appended to the
/// full file name, so `vault.wallet` becomes `vault.wallet.bak`.
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

/// Keeps a vault open in one app instance at a time.
///
/// Every save rewrites the whole file, so two windows editing the same vault
/// would silently undo each other. The lock is `<vault>.lock` holding the
/// owner's PID; a lock whose process is gone (a crash) is taken over. The
/// owning process may open the vault again (re-unlock, tests).
#[derive(Debug)]
pub struct VaultLock {
    path: PathBuf,
    owned: bool,
}

impl VaultLock {
    /// A lock that holds nothing: for a sandbox copy of a vault that never
    /// touches the file.
    pub fn detached() -> Self {
        Self {
            path: PathBuf::new(),
            owned: false,
        }
    }

    pub fn acquire(vault: &Path) -> Result<Self> {
        let mut name = vault.file_name().unwrap_or_default().to_os_string();
        name.push(".lock");
        let path = vault.with_file_name(name);
        let own = std::process::id();
        if let Some(holder) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
        {
            if holder == own {
                return Ok(Self { path, owned: false });
            }
            if process_alive(holder) {
                return Err(Error::VaultBusy(vault.display().to_string()));
            }
        }
        std::fs::write(&path, own.to_string()).map_err(|err| Error::Io(err.to_string()))?;
        Ok(Self { path, owned: true })
    }
}

impl Drop for VaultLock {
    fn drop(&mut self) {
        if self.owned {
            std::fs::remove_file(&self.path).ok();
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Without `/proc` a leftover lock cannot be told from a live one; it is
/// ignored rather than locking the user out after a crash.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn process_alive(_pid: u32) -> bool {
    false
}

/// File extension of a vault on disk.
pub const VAULT_EXT: &str = "wallet";

/// Append `.wallet` unless the name already ends with it (any case).
///
/// Native save dialogs on Linux return exactly what was typed, so `my-money`
/// would otherwise become an extension-less file the open dialog's filter hides.
pub fn ensure_wallet_ext(path: &Path) -> PathBuf {
    let has_ext = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(VAULT_EXT));
    if has_ext {
        return path.to_path_buf();
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(VAULT_EXT);
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_wallet_ext_appends_only_when_missing() {
        let cases = [
            ("/tmp/a", "/tmp/a.wallet"),
            ("/tmp/a.wallet", "/tmp/a.wallet"),
            ("/tmp/a.WALLET", "/tmp/a.WALLET"),
            ("/tmp/a.json", "/tmp/a.json.wallet"),
            ("/tmp/my.money", "/tmp/my.money.wallet"),
        ];
        for (input, expected) in cases {
            assert_eq!(ensure_wallet_ext(Path::new(input)), PathBuf::from(expected));
        }
    }

    #[test]
    fn a_vault_lock_keeps_other_processes_out() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        let lock_file = dir.path().join("v.wallet.lock");
        {
            let _held = VaultLock::acquire(&vault).unwrap();
            assert!(lock_file.exists());
            // The same process may open it again.
            assert!(VaultLock::acquire(&vault).is_ok());
            assert!(
                lock_file.exists(),
                "a re-entrant lock does not release the first"
            );
        }
        assert!(!lock_file.exists());
        // PID 1 is always alive on Linux: another process holds it.
        std::fs::write(&lock_file, "1").unwrap();
        if cfg!(any(target_os = "linux", target_os = "android")) {
            assert!(matches!(
                VaultLock::acquire(&vault),
                Err(Error::VaultBusy(_))
            ));
        }
        // A dead holder is taken over.
        std::fs::write(&lock_file, "4294967000").unwrap();
        assert!(VaultLock::acquire(&vault).is_ok());
    }
}
