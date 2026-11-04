//! A vault with or without a password: ask, set, remove.
//!
//! "Without a password" is a property of the file (see `crypto`): its header
//! says so and the payload is not encrypted. Setting or removing the password
//! rewrites the file at once, atomically, like `change_password`.

use std::sync::Arc;

use zeroize::Zeroizing;

use super::UnlockedVault;
use crate::crypto::SealKey;
use crate::error::{Error, Result};
use crate::storage::VaultFile;
use crate::vault_io::backups;

impl UnlockedVault {
    /// Whether the file is encrypted (the vault has a password).
    pub fn is_encrypted(&self) -> bool {
        self.key.as_ref().is_none_or(|key| !key.is_plain())
    }

    /// Give a vault without a password one. The file is rewritten encrypted
    /// first; only then, if `delete_plain_copies`, the vault's backups made
    /// without a password are deleted. Returns how many were deleted.
    pub fn set_password(&mut self, new_password: &str, delete_plain_copies: bool) -> Result<u32> {
        if self.sandbox {
            return Err(Error::Unsupported);
        }
        if self.is_encrypted() {
            return Err(Error::InvalidInput(
                "the vault already has a password: change it".into(),
            ));
        }
        if new_password.is_empty() {
            return Err(Error::InvalidInput(
                "the new password must not be empty".into(),
            ));
        }
        self.rekey(SealKey::fresh(new_password.as_bytes())?, new_password)?;
        if delete_plain_copies {
            backups::remove_plain(&self.path)
        } else {
            Ok(0)
        }
    }

    /// Take the password off. The current password is checked against the
    /// file on disk first (a wrong one is `SECRET` and nothing changes); the
    /// file is then rewritten without encryption. The pre-sync copies are
    /// deleted (they are sealed under the password just removed and nothing
    /// could open them any more); other backups stay as they are, encrypted
    /// under the password they were made with.
    pub fn remove_password(&mut self, current_password: &str) -> Result<()> {
        if self.sandbox {
            return Err(Error::Unsupported);
        }
        if !self.is_encrypted() {
            return Err(Error::InvalidInput("the vault has no password".into()));
        }
        VaultFile::open(&self.path, current_password)?;
        self.rekey(SealKey::plain(), "")?;
        // The pre-sync copies are sealed under the password just removed: the
        // restore button would offer a copy nothing can open.
        self.remove_sync_backups();
        Ok(())
    }

    /// Switch to `key` and write the file under it now; on a failed write the
    /// old key and password stay, matching the unchanged file.
    fn rekey(&mut self, key: SealKey, password: &str) -> Result<()> {
        let old_key = self.key.replace(Arc::new(key));
        let old_password = std::mem::replace(&mut self.password, Zeroizing::new(password.into()));
        if let Err(err) = self.persist_now() {
            self.key = old_key;
            self.password = old_password;
            return Err(err);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::empty_vault;
    use super::*;
    use crate::crypto::file_is_encrypted;
    use tempfile::tempdir;

    fn plain_vault() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempdir().unwrap();
        let vault = UnlockedVault::create_with(dir.path().join("v.wallet"), None).unwrap();
        (dir, vault)
    }

    #[test]
    fn a_vault_without_a_password_is_written_and_read_in_the_clear() {
        let (dir, mut vault) = plain_vault();
        assert!(!vault.is_encrypted());
        vault.persist_now().unwrap();
        assert!(!file_is_encrypted(&vault.path));
        let id = vault.doc.id.clone();
        drop(vault);
        // Whatever is typed opens it; the reopened vault stays without one.
        for typed in ["", "anything"] {
            let reopened = UnlockedVault::open(dir.path().join("v.wallet"), typed).unwrap();
            assert_eq!(reopened.doc.id, id);
            assert!(!reopened.is_encrypted());
            assert!(reopened.check_password("whatever").is_ok());
        }
        let bytes = std::fs::read(dir.path().join("v.wallet")).unwrap();
        assert!(
            bytes
                .windows(id.len())
                .any(|window| window == id.as_bytes())
        );
    }

    #[test]
    fn a_damaged_plain_file_is_refused_not_misread() {
        let (dir, vault) = plain_vault();
        let path = vault.path.clone();
        drop(vault);
        let mut bytes = std::fs::read(&path).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        assert!(matches!(
            UnlockedVault::open(path, ""),
            Err(Error::Deserialize(_))
        ));
        drop(dir);
    }

    #[test]
    fn a_password_can_be_set_and_removed_back_and_forth() {
        let (dir, mut vault) = plain_vault();
        let path = vault.path.clone();
        assert_eq!(vault.set_password("one", false).unwrap(), 0);
        assert!(vault.is_encrypted() && file_is_encrypted(&path));
        assert!(matches!(
            vault.set_password("two", false),
            Err(Error::InvalidInput(_))
        ));
        vault.remove_password("one").unwrap();
        assert!(!vault.is_encrypted() && !file_is_encrypted(&path));
        assert!(matches!(
            vault.remove_password("one"),
            Err(Error::InvalidInput(_))
        ));
        vault.set_password("three", false).unwrap();
        drop(vault);
        assert!(matches!(
            UnlockedVault::open(path.clone(), "one"),
            Err(Error::Secret)
        ));
        assert!(UnlockedVault::open(path, "three").unwrap().is_encrypted());
        drop(dir);
    }

    #[test]
    fn removing_the_password_deletes_the_copies_sealed_under_it() {
        let (dir, mut vault) = empty_vault();
        vault
            .add_transaction(crate::service::test_support::income_of(
                &vault.doc.stores[0].id.clone(),
                crate::service::test_support::usd(),
                "5",
                "1",
                "before the sync",
            ))
            .unwrap();
        vault.sync_backup().unwrap();
        vault.sync_backup().unwrap();
        assert!(vault.has_sync_backup());
        vault.remove_password("pw").unwrap();
        // The button hides, the copies are gone and a restore says so instead
        // of failing on the empty password.
        assert!(!vault.has_sync_backup());
        let left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".presync"))
            .collect();
        assert!(left.is_empty(), "left behind: {left:?}");
        assert!(matches!(
            vault.sync_restore_backup(),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn a_copy_that_the_current_state_cannot_open_is_not_offered() {
        let (_dir, mut vault) = plain_vault();
        // An encrypted copy next to a vault without a password: nothing to restore from.
        let (other_dir, other) = empty_vault();
        let sealed = std::fs::read(&other.path).unwrap();
        drop(other);
        drop(other_dir);
        let copy = vault.path.with_file_name("v.wallet.presync.bak");
        std::fs::write(&copy, sealed).unwrap();
        assert!(copy.exists());
        assert!(!vault.has_sync_backup());
        assert!(matches!(
            vault.sync_restore_backup(),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn setting_a_password_keeps_the_copy_from_before_a_sync_readable() {
        let (_dir, mut vault) = plain_vault();
        vault.sync_backup().unwrap();
        vault.set_password("pw", false).unwrap();
        assert!(vault.has_sync_backup());
        vault.sync_restore_backup().unwrap();
        assert!(vault.is_encrypted());
    }

    #[test]
    fn removing_with_a_wrong_password_changes_nothing() {
        let (dir, mut vault) = empty_vault();
        let before = std::fs::read(&vault.path).unwrap();
        assert!(matches!(vault.remove_password("nope"), Err(Error::Secret)));
        assert!(matches!(vault.remove_password(""), Err(Error::Secret)));
        assert!(vault.is_encrypted());
        assert_eq!(std::fs::read(&vault.path).unwrap(), before);
        // The session still saves under the old key.
        vault.persist_now().unwrap();
        assert!(file_is_encrypted(&vault.path));
        drop(vault);
        assert!(UnlockedVault::open(dir.path().join("test.wallet"), "pw").is_ok());
    }

    #[test]
    fn an_empty_password_is_refused_everywhere() {
        let dir = tempdir().unwrap();
        assert!(matches!(
            UnlockedVault::create_with(dir.path().join("a.wallet"), Some("")),
            Err(Error::InvalidInput(_))
        ));
        let (_dir, mut vault) = plain_vault();
        assert!(matches!(
            vault.set_password("", false),
            Err(Error::InvalidInput(_))
        ));
        assert!(!vault.is_encrypted());
        let (_dir2, mut locked) = empty_vault();
        assert!(matches!(
            locked.change_password("pw", ""),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn a_vault_without_a_password_has_none_to_change() {
        let (_dir, mut vault) = plain_vault();
        assert!(matches!(
            vault.change_password("", "new"),
            Err(Error::InvalidInput(_))
        ));
        assert!(!vault.is_encrypted());
    }

    #[test]
    fn setting_a_password_deletes_only_this_vaults_plain_copies_on_request() {
        let (dir, mut vault) = plain_vault();
        let folder = dir.path();
        let plain = vault.create_backup().unwrap();
        // A copy under a password, another vault's plain copy, and a stray file.
        let encrypted = {
            let (other_dir, other) = empty_vault();
            let bytes = std::fs::read(&other.path).unwrap();
            drop(other);
            drop(other_dir);
            bytes
        };
        std::fs::write(folder.join("v.wallet.v2.bak"), &encrypted).unwrap();
        let foreign = std::fs::read(&vault.path).unwrap();
        std::fs::write(folder.join("other.wallet.bak"), &foreign).unwrap();
        std::fs::write(folder.join("v.wallet.notes.txt"), &foreign).unwrap();
        std::fs::write(folder.join("v.wallet.junk.bak"), b"not a vault").unwrap();

        assert_eq!(vault.set_password("pw", true).unwrap(), 1);
        assert!(!folder.join(&plain.name).exists());
        for kept in [
            "v.wallet",
            "v.wallet.v2.bak",
            "other.wallet.bak",
            "v.wallet.notes.txt",
            "v.wallet.junk.bak",
        ] {
            assert!(folder.join(kept).exists(), "{kept} must stay");
        }
        assert!(file_is_encrypted(&folder.join("v.wallet")));
    }

    #[test]
    fn setting_a_password_keeps_plain_copies_unless_asked() {
        let (dir, mut vault) = plain_vault();
        let plain = vault.create_backup().unwrap();
        assert_eq!(vault.set_password("pw", false).unwrap(), 0);
        assert!(dir.path().join(plain.name).exists());
    }

    #[test]
    fn the_old_encrypted_format_still_reads() {
        // A file written before the plain variant existed: kdf_id 0, AEAD.
        let (dir, vault) = empty_vault();
        drop(vault);
        let (doc, _) = VaultFile::open(&dir.path().join("test.wallet"), "pw").unwrap();
        assert!(!doc.id.is_empty());
    }
}
