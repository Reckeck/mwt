//! Moving the app's own directories when its identifier changes (TradeApp →
//! Multi Wallet Tracker): settings, the default vault and the webview data
//! follow the user instead of staying behind under the old name.

use std::io;
use std::path::{Path, PathBuf};

/// Move `old` to `new` unless there is nothing to move or `new` already holds
/// something (then the new place wins and `old` is left alone).
/// `Ok(true)` when it moved.
pub fn move_legacy_dir(old: &Path, new: &Path) -> io::Result<bool> {
    if old == new || !old.is_dir() {
        return Ok(false);
    }
    if new.exists() {
        if !is_empty_dir(new)? {
            return Ok(false);
        }
        std::fs::remove_dir(new)?;
    }
    if let Some(parent) = new.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Same parent directory almost always, so a rename; a copy covers the rest.
    if std::fs::rename(old, new).is_err() {
        copy_dir(old, new)?;
        std::fs::remove_dir_all(old)?;
    }
    Ok(true)
}

/// `path` with a moved directory's prefix replaced, if it lay under one.
pub fn rebase_path(path: &str, moves: &[(PathBuf, PathBuf)]) -> Option<String> {
    let path = Path::new(path);
    moves.iter().find_map(|(old, new)| {
        path.strip_prefix(old)
            .ok()
            .map(|rest| new.join(rest).to_string_lossy().into_owned())
    })
}

fn is_empty_dir(dir: &Path) -> io::Result<bool> {
    Ok(dir.is_dir() && std::fs::read_dir(dir)?.next().is_none())
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_the_old_directory_with_its_contents() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("dev.tradeapp.desktop");
        let new = root.path().join("dev.mwt.desktop");
        std::fs::create_dir_all(old.join("nested")).unwrap();
        std::fs::write(old.join("vault.wallet"), b"sealed").unwrap();
        std::fs::write(old.join("nested/settings.json"), b"{}").unwrap();

        assert!(move_legacy_dir(&old, &new).unwrap());
        assert!(!old.exists());
        assert_eq!(std::fs::read(new.join("vault.wallet")).unwrap(), b"sealed");
        assert!(new.join("nested/settings.json").exists());
    }

    #[test]
    fn an_empty_new_directory_does_not_block_the_move() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("old");
        let new = root.path().join("new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("file"), b"x").unwrap();
        std::fs::create_dir_all(&new).unwrap();

        assert!(move_legacy_dir(&old, &new).unwrap());
        assert!(new.join("file").exists());
    }

    #[test]
    fn existing_new_data_is_never_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("old");
        let new = root.path().join("new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("vault.wallet"), b"old").unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("vault.wallet"), b"new").unwrap();

        assert!(!move_legacy_dir(&old, &new).unwrap());
        assert_eq!(std::fs::read(new.join("vault.wallet")).unwrap(), b"new");
        assert_eq!(std::fs::read(old.join("vault.wallet")).unwrap(), b"old");
    }

    #[test]
    fn nothing_to_move_is_not_an_error() {
        let root = tempfile::tempdir().unwrap();
        let moved =
            move_legacy_dir(&root.path().join("missing"), &root.path().join("new")).unwrap();
        assert!(!moved);
        assert!(!root.path().join("new").exists());
    }

    #[test]
    fn paths_under_a_moved_directory_are_rebased() {
        let moves = vec![(
            PathBuf::from("/home/user/.local/share/dev.tradeapp.desktop"),
            PathBuf::from("/home/user/.local/share/dev.mwt.desktop"),
        )];
        assert_eq!(
            rebase_path(
                "/home/user/.local/share/dev.tradeapp.desktop/vault.wallet",
                &moves
            )
            .as_deref(),
            Some("/home/user/.local/share/dev.mwt.desktop/vault.wallet"),
        );
        assert_eq!(rebase_path("/home/user/Documents/my.wallet", &moves), None);
        // A sibling whose name only starts the same is not inside the moved dir.
        assert_eq!(
            rebase_path("/home/user/.local/share/dev.tradeapp.desktop2/a", &moves),
            None
        );
    }
}
