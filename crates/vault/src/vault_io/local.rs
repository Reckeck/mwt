//! The vault file on a plain file system: atomic replace, the instance lock
//! and the backup slots next to the file. Every platform keeps the *working*
//! vault on such a path (on Android and iOS inside the app's own storage), so
//! this is the only code that ever writes a vault.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use mwt_base::{Error, Result};

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

    let result = write_synced(&tmp_path, bytes).and_then(|()| replace_with(&tmp_path, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

/// The first half of a replace: `bytes` are on the disk at `tmp`, which must
/// be on the same file system as the file it will replace.
pub fn write_synced(tmp: &Path, bytes: &[u8]) -> Result<()> {
    create_synced(tmp, bytes).map(drop)
}

/// [`write_synced`] that keeps the file open for reading it back. `tmp` must
/// not exist yet: `create_new` refuses an existing file and a planted
/// symlink, so what gets written is a file this call made.
pub fn create_synced(tmp: &Path, bytes: &[u8]) -> Result<File> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(tmp)?;
    // From here the file is this call's own: a failed write removes it.
    match file.write_all(bytes).and_then(|()| file.sync_all()) {
        Ok(()) => Ok(file),
        Err(err) => {
            drop(file);
            let _ = std::fs::remove_file(tmp);
            Err(err.into())
        }
    }
}

/// Everything in the open `file`, from the start.
pub fn read_back(file: &mut File) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Whether `tmp` is still the regular file behind `file` and holds exactly
/// `bytes`: checked right before a rename, so what takes a vault's place is
/// what was verified, not a file or symlink swapped in after.
pub fn still_the_written_file(file: &File, tmp: &Path, bytes: &[u8]) -> Result<()> {
    let swapped = || Error::Io(format!("{} changed after it was verified", tmp.display()));
    let meta = std::fs::symlink_metadata(tmp)?;
    if !meta.file_type().is_file() {
        return Err(swapped());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let held = file.metadata()?;
        if (held.dev(), held.ino()) != (meta.dev(), meta.ino()) {
            return Err(swapped());
        }
    }
    #[cfg(not(unix))]
    let _ = file;
    if std::fs::read(tmp)? != bytes {
        return Err(swapped());
    }
    Ok(())
}

const REENCRYPT_TAG: &str = ".reencrypt.";

/// `vault.wallet` → `.vault.wallet.reencrypt.<pid>.<random>.tmp`, in the
/// same directory (the rename that finishes the job works only within one
/// file system). Unique per run, so two runs never share a file.
pub fn reencrypt_tmp_path(path: &Path) -> PathBuf {
    let mut name = reencrypt_prefix(path);
    name.push_str(&format!(
        "{}.{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    path.with_file_name(name)
}

fn reencrypt_prefix(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    format!(".{name}{REENCRYPT_TAG}")
}

/// The pid in a re-encryption temp name of `path`'s vault, or `None` when
/// `name` is anything else.
fn reencrypt_leftover_pid(path: &Path, name: &str) -> Option<u32> {
    let rest = name.strip_prefix(&reencrypt_prefix(path))?;
    let rest = rest.strip_suffix(".tmp")?;
    let (pid, random) = rest.split_once('.')?;
    let random_ok = random.len() == 32 && random.bytes().all(|byte| byte.is_ascii_hexdigit());
    let pid_ok = !pid.is_empty() && pid.bytes().all(|byte| byte.is_ascii_digit());
    if random_ok && pid_ok {
        pid.parse().ok()
    } else {
        None
    }
}

/// Removes the re-encryption temp files a crash left next to `path` and says
/// how many went. Only regular files with exactly the temp scheme's name;
/// a run of this very process is left alone (it cleans up after itself).
/// Best-effort: a file that cannot be removed stays for the next time.
pub fn sweep_reencrypt_leftovers(path: &Path) -> usize {
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let own = std::process::id();
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| reencrypt_leftover_pid(path, name))
                .is_some_and(|pid| pid != own)
        })
        .filter(|entry| std::fs::remove_file(entry.path()).is_ok())
        .count()
}

/// The second half: `tmp` takes the place of `path` in one step.
pub fn replace_with(tmp: &Path, path: &Path) -> Result<()> {
    std::fs::rename(tmp, path)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    if let Ok(handle) = File::open(dir) {
        let _ = handle.sync_all(); // best-effort dir fsync
    }
    Ok(())
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
/// would silently undo each other. The lock is an OS advisory lock on
/// `<vault>.lock`: the OS releases it when the process ends, so a crash
/// leaves nothing stale and the file content is never read. The owning
/// process may open the vault again (re-unlock, tests); the lock is released
/// when the last handle is dropped.
#[derive(Debug)]
pub struct VaultLock {
    _held: Option<Arc<File>>,
}

/// The locks this process holds, so it can share one instead of locking the
/// same file twice (two open file descriptions of one file exclude each other).
static HELD: Mutex<BTreeMap<PathBuf, Weak<File>>> = Mutex::new(BTreeMap::new());

/// The key of a lock file in [`HELD`]: its directory made canonical, so two
/// spellings of one path share a lock. The raw path when that fails.
fn held_key(lock_file: &Path) -> PathBuf {
    let Some(name) = lock_file.file_name() else {
        return lock_file.to_path_buf();
    };
    let parent = match lock_file.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    std::fs::canonicalize(parent).map_or_else(|_| lock_file.to_path_buf(), |dir| dir.join(name))
}

impl VaultLock {
    /// A lock that holds nothing: for a sandbox copy of a vault that never
    /// touches the file.
    pub fn detached() -> Self {
        Self { _held: None }
    }

    pub fn acquire(vault: &Path) -> Result<Self> {
        let mut name = vault.file_name().unwrap_or_default().to_os_string();
        name.push(".lock");
        let path = vault.with_file_name(name);
        let key = held_key(&path);
        let mut held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(shared) = held.get(&key).and_then(Weak::upgrade) {
            return Ok(Self {
                _held: Some(shared),
            });
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|err| Error::Io(err.to_string()))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(Error::VaultBusy(vault.display().to_string()));
            }
            Err(TryLockError::Error(err)) => return Err(Error::Io(err.to_string())),
        }
        let file = Arc::new(file);
        held.retain(|_, weak| weak.strong_count() > 0);
        held.insert(key, Arc::downgrade(&file));
        Ok(Self { _held: Some(file) })
    }
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
pub(crate) mod tests {
    use super::*;

    /// A temp name left by another process (this one is never swept).
    pub(crate) fn foreign_reencrypt_leftover(vault: &Path) -> PathBuf {
        let mine = reencrypt_tmp_path(vault);
        let name = mine.file_name().unwrap().to_str().unwrap();
        let own = format!(".{}.", std::process::id());
        let foreign = name.replacen(&own, &format!(".{}.", std::process::id() ^ 1), 1);
        vault.with_file_name(foreign)
    }

    #[test]
    fn every_reencryption_gets_its_own_temp_name_next_to_the_vault() {
        let vault = Path::new("/data/v.wallet");
        let first = reencrypt_tmp_path(vault);
        let second = reencrypt_tmp_path(vault);
        assert_ne!(first, second);
        assert_eq!(first.parent(), vault.parent());
        let name = first.file_name().unwrap().to_str().unwrap();
        assert_eq!(
            reencrypt_leftover_pid(vault, name),
            Some(std::process::id())
        );
    }

    #[test]
    fn a_temp_file_is_never_written_through_an_existing_name_or_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let taken = dir.path().join("taken");
        std::fs::write(&taken, b"someone else's").unwrap();
        assert!(create_synced(&taken, b"new").is_err());
        assert_eq!(std::fs::read(&taken).unwrap(), b"someone else's");
        #[cfg(unix)]
        {
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&taken, &link).unwrap();
            assert!(create_synced(&link, b"new").is_err());
            assert_eq!(std::fs::read(&taken).unwrap(), b"someone else's");
        }
    }

    #[test]
    fn a_temp_file_swapped_after_writing_is_caught_before_the_rename() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("t");
        let mut file = create_synced(&tmp, b"verified").unwrap();
        assert_eq!(read_back(&mut file).unwrap(), b"verified");
        still_the_written_file(&file, &tmp, b"verified").unwrap();

        std::fs::write(&tmp, b"changed!").unwrap();
        assert!(still_the_written_file(&file, &tmp, b"verified").is_err());

        std::fs::remove_file(&tmp).unwrap();
        std::fs::write(&tmp, b"verified").unwrap();
        if cfg!(unix) {
            assert!(
                still_the_written_file(&file, &tmp, b"verified").is_err(),
                "another inode"
            );
        }
        std::fs::remove_file(&tmp).unwrap();
        assert!(still_the_written_file(&file, &tmp, b"verified").is_err());
    }

    #[test]
    fn the_sweep_removes_only_reencryption_leftovers_of_this_vault() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        std::fs::write(&vault, b"vault").unwrap();
        let leftover = foreign_reencrypt_leftover(&vault);
        let second = foreign_reencrypt_leftover(&vault);
        std::fs::write(&leftover, b"half").unwrap();
        std::fs::write(&second, b"half").unwrap();
        let running = reencrypt_tmp_path(&vault);
        std::fs::write(&running, b"in progress").unwrap();
        let kept = [
            dir.path().join("v.wallet.bak"),
            dir.path()
                .join(".v.wallet.tmp.12.0123456789abcdef0123456789abcdef"),
            dir.path().join(".v.wallet.reencrypt.tmp"),
            dir.path().join(".v.wallet.reencrypt.12.short.tmp"),
            dir.path()
                .join(".v.wallet.reencrypt.x1.0123456789abcdef0123456789abcdef.tmp"),
            dir.path()
                .join(".other.wallet.reencrypt.12.0123456789abcdef0123456789abcdef.tmp"),
        ];
        for file in &kept {
            std::fs::write(file, b"keep").unwrap();
        }
        let leftover_dir = foreign_reencrypt_leftover(&dir.path().join("w.wallet"));
        std::fs::create_dir(
            leftover_dir.with_file_name(
                leftover_dir
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replacen(".w.", ".v.", 1),
            ),
        )
        .unwrap();

        assert_eq!(sweep_reencrypt_leftovers(&vault), 2);
        assert!(!leftover.exists() && !second.exists());
        assert!(vault.exists() && running.exists());
        assert!(kept.iter().all(|file| file.exists()));
        assert_eq!(sweep_reencrypt_leftovers(&vault), 0);
    }

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

    fn lock_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("v.wallet.lock")
    }

    #[test]
    fn the_same_process_may_acquire_again() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        let first = VaultLock::acquire(&vault).unwrap();
        let second = VaultLock::acquire(&vault).unwrap();
        drop(first);
        assert!(
            VaultLock::acquire(&vault).is_ok(),
            "the second handle still holds the shared lock"
        );
        drop(second);
    }

    #[test]
    fn another_spelling_of_the_path_reuses_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        let respelled = dir.path().join(".").join("v.wallet");
        let first = VaultLock::acquire(&vault).unwrap();
        let second = VaultLock::acquire(&respelled);
        assert!(second.is_ok(), "one vault, one lock: {second:?}");
        drop(first);
    }

    #[test]
    fn dropping_the_last_handle_releases_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        drop(VaultLock::acquire(&vault).unwrap());
        let other = File::options().write(true).open(lock_path(&dir)).unwrap();
        assert!(other.try_lock().is_ok());
    }

    #[test]
    fn a_second_instance_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        let other_instance = File::create(lock_path(&dir)).unwrap();
        other_instance.try_lock().unwrap();
        assert!(matches!(
            VaultLock::acquire(&vault),
            Err(Error::VaultBusy(_))
        ));
        drop(other_instance);
        assert!(VaultLock::acquire(&vault).is_ok());
    }

    #[test]
    fn a_crash_leaves_no_stale_lock() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        // A crashed instance leaves its lock file behind, held by nobody. The file
        // names a live process (PID 1): the old PID scheme refused it (on Linux),
        // the OS lock does not care, since nobody holds the lock.
        std::fs::write(lock_path(&dir), "1").unwrap();
        assert!(VaultLock::acquire(&vault).is_ok());
    }

    #[test]
    fn a_stale_pid_file_from_the_old_scheme_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        std::fs::write(lock_path(&dir), "4294967000").unwrap();
        assert!(VaultLock::acquire(&vault).is_ok());
        std::fs::write(lock_path(&dir), "1").unwrap();
        drop(VaultLock::acquire(&vault));
        assert!(VaultLock::acquire(&vault).is_ok());
    }
}
