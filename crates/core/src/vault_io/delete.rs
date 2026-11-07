//! Deleting a vault that lives in the app's own folder (`vaults_dir`), and
//! telling whether a path does. The path is checked here, not in the UI: the
//! file must really be a vault inside the folder, whatever the path looks like.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::VAULT_MAGIC;
use crate::error::{Error, Result};
use crate::vault_io::backups;

/// What [`delete_in_app_folder`] removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedVault {
    /// Name of the vault file.
    pub file_name: String,
    /// Names of the backup copies that went with it.
    pub backups: Vec<String>,
}

/// Paths the deletion must keep out of: the vault that is open and the sandbox.
#[derive(Debug, Default, Clone, Copy)]
pub struct Protected<'a> {
    pub open_vault: Option<&'a Path>,
    pub sandbox: Option<&'a Path>,
}

/// The canonical path of `path` when it is a file strictly inside `dir`
/// (not `dir` itself), reached without `..` and not through a link.
pub fn inside_folder(dir: &Path, path: &Path) -> Option<PathBuf> {
    if path.components().any(|part| part == Component::ParentDir) {
        return None;
    }
    if std::fs::symlink_metadata(path)
        .ok()?
        .file_type()
        .is_symlink()
    {
        return None;
    }
    let root = std::fs::canonicalize(dir).ok()?;
    let real = std::fs::canonicalize(path).ok()?;
    (real != root && real.starts_with(&root) && real.is_file()).then_some(real)
}

/// Whether the file starts with the vault magic.
fn has_vault_magic(path: &Path) -> bool {
    let mut head = [0u8; VAULT_MAGIC.len()];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .is_ok()
        && &head == VAULT_MAGIC
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(first), Ok(second)) => first == second,
        _ => left == right,
    }
}

/// Delete the vault file `path` and its backup copies. Refused with
/// `INVALID_INPUT` (and nothing removed) unless `path` is a vault file inside
/// `dir`, is not the open vault and is not inside the sandbox.
pub fn delete_in_app_folder(
    dir: &Path,
    path: &Path,
    protected: Protected<'_>,
) -> Result<DeletedVault> {
    if !path.exists() {
        return Err(Error::NotFound(path.to_string_lossy().into_owned()));
    }
    let real = inside_folder(dir, path)
        .ok_or_else(|| Error::InvalidInput("the file is not in the app folder".into()))?;
    if !has_vault_magic(&real) {
        return Err(Error::InvalidInput("the file is not a vault".into()));
    }
    if protected
        .open_vault
        .is_some_and(|open| same_file(open, &real))
    {
        return Err(Error::InvalidInput(
            "the open vault cannot be deleted".into(),
        ));
    }
    if protected.sandbox.is_some_and(|root| {
        std::fs::canonicalize(root).is_ok_and(|root| real.starts_with(root))
            || real.starts_with(root)
    }) {
        return Err(Error::InvalidInput("the sandbox cannot be deleted".into()));
    }
    let file_name = real
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Resolve every copy before anything goes, so a bad one stops it all.
    let copies = backups::list(&real)?
        .into_iter()
        .map(|copy| backups::resolve(&real, &copy.name).map(|found| (copy.name, found)))
        .collect::<Result<Vec<_>>>()?;
    std::fs::remove_file(&real).map_err(|err| Error::Io(err.to_string()))?;
    let mut removed = Vec::new();
    for (name, found) in copies {
        std::fs::remove_file(&found).map_err(|err| Error::Io(err.to_string()))?;
        removed.push(name);
    }
    // A raise to the crypto level that crashed midway leaves a sealed copy.
    crate::vault_io::local::sweep_reencrypt_leftovers(&real);
    Ok(DeletedVault {
        file_name,
        backups: removed,
    })
}

/// `path` must be an existing file that starts with the vault magic, else `InvalidInput`
/// (a history row may only point at a vault).
pub fn ensure_vault_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err(Error::InvalidInput("the file does not exist".into()));
    }
    if !has_vault_magic(path) {
        return Err(Error::InvalidInput("the file is not a vault".into()));
    }
    Ok(())
}

/// The bytes of the vault file `path`, for a copy: refused unless it is a
/// vault (magic header).
pub fn read_vault_bytes(path: &Path) -> Result<Vec<u8>> {
    if !has_vault_magic(path) {
        return Err(Error::InvalidInput("the file is not a vault".into()));
    }
    std::fs::read(path).map_err(|err| Error::Io(err.to_string()))
}

/// Whether `target` is the same file as `source` (a copy onto itself).
pub fn is_same_file(source: &Path, target: &Path) -> bool {
    same_file(source, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_vault(path: &Path) {
        let mut bytes = VAULT_MAGIC.to_vec();
        bytes.extend_from_slice(b"payload");
        std::fs::write(path, bytes).unwrap();
    }

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("vaults");
        std::fs::create_dir(&dir).unwrap();
        (root, dir)
    }

    #[test]
    fn deleting_a_vault_sweeps_its_reencryption_leftovers() {
        let (_root, dir) = setup();
        let vault = dir.join("a.wallet");
        let other = dir.join("b.wallet");
        write_vault(&vault);
        write_vault(&other);
        let leftover = crate::vault_io::local::tests::foreign_reencrypt_leftover(&vault);
        let not_ours = crate::vault_io::local::tests::foreign_reencrypt_leftover(&other);
        std::fs::write(&leftover, b"sealed copy").unwrap();
        std::fs::write(&not_ours, b"sealed copy").unwrap();
        delete_in_app_folder(&dir, &vault, Protected::default()).unwrap();
        assert!(!vault.exists() && !leftover.exists());
        assert!(other.exists() && not_ours.exists());
    }

    #[test]
    fn deletes_vault_and_its_copies_only() {
        let (_root, dir) = setup();
        let vault = dir.join("a.wallet");
        let other = dir.join("b.wallet");
        write_vault(&vault);
        write_vault(&other);
        std::fs::write(dir.join("a.wallet.bak"), b"x").unwrap();
        std::fs::write(dir.join("a.wallet.20261022-094000.bak"), b"x").unwrap();
        std::fs::write(dir.join("b.wallet.bak"), b"x").unwrap();
        let done = delete_in_app_folder(&dir, &vault, Protected::default()).unwrap();
        assert_eq!(done.file_name, "a.wallet");
        assert_eq!(done.backups.len(), 2);
        assert!(!vault.exists());
        assert!(!dir.join("a.wallet.bak").exists());
        assert!(other.exists());
        assert!(dir.join("b.wallet.bak").exists());
    }

    fn refused(dir: &Path, path: &Path, protected: Protected<'_>) {
        let err = delete_in_app_folder(dir, path, protected).unwrap_err();
        assert_eq!(err.code(), "INVALID_INPUT", "{err}");
    }

    #[test]
    fn path_outside_the_folder_is_refused() {
        let (root, dir) = setup();
        let outside = root.path().join("out.wallet");
        write_vault(&outside);
        refused(&dir, &outside, Protected::default());
        assert!(outside.exists());
    }

    #[test]
    fn dot_dot_is_refused() {
        let (root, dir) = setup();
        let outside = root.path().join("out.wallet");
        write_vault(&outside);
        refused(
            &dir,
            &dir.join("..").join("out.wallet"),
            Protected::default(),
        );
        assert!(outside.exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_out_of_the_folder_is_refused() {
        let (root, dir) = setup();
        let outside = root.path().join("out.wallet");
        write_vault(&outside);
        let link = dir.join("link.wallet");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        refused(&dir, &link, Protected::default());
        assert!(outside.exists());
        assert!(link.exists());
    }

    #[test]
    fn the_folder_itself_is_refused() {
        let (_root, dir) = setup();
        refused(&dir, &dir, Protected::default());
        assert!(dir.exists());
    }

    #[test]
    fn a_file_that_is_not_a_vault_is_refused() {
        let (_root, dir) = setup();
        let note = dir.join("notes.wallet");
        std::fs::write(&note, b"hello, not a vault").unwrap();
        refused(&dir, &note, Protected::default());
        assert!(note.exists());
    }

    #[test]
    fn the_open_vault_is_refused() {
        let (_root, dir) = setup();
        let vault = dir.join("a.wallet");
        write_vault(&vault);
        std::fs::write(dir.join("a.wallet.bak"), b"x").unwrap();
        let open = Protected {
            open_vault: Some(&vault),
            sandbox: None,
        };
        refused(&dir, &vault, open);
        assert!(vault.exists());
        assert!(dir.join("a.wallet.bak").exists());
    }

    #[test]
    fn the_sandbox_is_refused() {
        let (_root, dir) = setup();
        let sandbox = dir.join("sandbox");
        std::fs::create_dir(&sandbox).unwrap();
        let vault = sandbox.join("sandbox.wallet");
        write_vault(&vault);
        let guard = Protected {
            open_vault: None,
            sandbox: Some(&sandbox),
        };
        refused(&dir, &vault, guard);
        assert!(vault.exists());
    }

    #[test]
    fn a_missing_file_is_not_found() {
        let (_root, dir) = setup();
        let err =
            delete_in_app_folder(&dir, &dir.join("no.wallet"), Protected::default()).unwrap_err();
        assert_eq!(err.code(), "NOT_FOUND");
    }

    #[test]
    fn inside_folder_tells_the_place() {
        let (root, dir) = setup();
        let vault = dir.join("a.wallet");
        write_vault(&vault);
        let outside = root.path().join("o.wallet");
        write_vault(&outside);
        assert!(inside_folder(&dir, &vault).is_some());
        assert!(inside_folder(&dir, &outside).is_none());
        assert!(inside_folder(&dir, &dir).is_none());
    }

    #[test]
    fn reading_bytes_needs_a_vault() {
        let (_root, dir) = setup();
        let vault = dir.join("a.wallet");
        write_vault(&vault);
        assert!(read_vault_bytes(&vault).unwrap().starts_with(VAULT_MAGIC));
        let note = dir.join("n.txt");
        std::fs::write(&note, b"no").unwrap();
        assert!(read_vault_bytes(&note).is_err());
        assert!(is_same_file(&vault, &dir.join(".").join("a.wallet")));
    }
}
