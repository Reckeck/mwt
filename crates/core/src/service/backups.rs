//! Backup copies of the open vault file: list, take one now, restore one.

use super::UnlockedVault;
use crate::error::{Error, Result};
use crate::storage::VaultFile;
use crate::vault_io::backups::{self, BackupFile};

impl UnlockedVault {
    /// The vault's backups next to the file, newest first.
    pub fn list_backups(&self) -> Result<Vec<BackupFile>> {
        backups::list(&self.path)
    }

    /// Copy the file as it is now (batched changes written first) to a new
    /// backup that never replaces an earlier one.
    pub fn create_backup(&mut self) -> Result<BackupFile> {
        if self.sandbox {
            return Err(Error::Unsupported);
        }
        self.flush_pending()?;
        backups::create(&self.path, &backups::stamp(time::OffsetDateTime::now_utc()))
    }

    /// Replace the content of the vault with the backup called `name`; with
    /// live peers, write the copy's records over the current ones and delete
    /// nothing.
    ///
    /// The backup is read with this session's password first (a copy made under
    /// another password is refused with `SECRET` and nothing changes), then the
    /// current file is copied to a new backup of its own, so the restore itself
    /// can be taken back. Returns that safety copy.
    pub fn restore_backup(&mut self, name: &str) -> Result<BackupFile> {
        if self.sandbox {
            return Err(Error::Unsupported);
        }
        let source = backups::resolve(&self.path, name)?;
        let (doc, _) = VaultFile::open(&source, &self.password)?;
        let safety = self.create_backup()?;
        if self.has_live_peers() {
            // Other devices keep what they hold: the copy's records come back
            // by value and nothing is deleted, so a record made after the
            // copy (here or on a peer) stays.
            self.apply_records_of(&doc)?;
        } else {
            self.replace_doc(doc)?;
        }
        Ok(safety)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::error::Error;
    use crate::service::UnlockedVault;

    fn vault_with_store(path: std::path::PathBuf, name: &str) -> UnlockedVault {
        let mut vault = UnlockedVault::create(path, "pw").unwrap();
        vault
            .create_store(
                name,
                crate::model::store::StoreKind::Bank,
                "#000000",
                "",
                None,
            )
            .unwrap();
        vault
    }

    #[test]
    fn a_backup_taken_now_is_listed_and_restores_the_older_state() {
        let dir = tempdir().unwrap();
        let mut vault = vault_with_store(dir.path().join("v.wallet"), "Before");
        let backup = vault.create_backup().unwrap();
        assert!(backup.name.starts_with("v.wallet.") && backup.name.ends_with(".bak"));
        assert_eq!(vault.list_backups().unwrap()[0].name, backup.name);

        vault
            .create_store(
                "After",
                crate::model::store::StoreKind::Bank,
                "#000000",
                "",
                None,
            )
            .unwrap();
        let stores = |vault: &UnlockedVault| vault.doc.stores.len();
        let with_both = stores(&vault);

        let safety = vault.restore_backup(&backup.name).unwrap();
        assert_eq!(stores(&vault), with_both - 1);
        // The state before the restore is kept, and can be restored in turn.
        vault.restore_backup(&safety.name).unwrap();
        assert_eq!(stores(&vault), with_both);
        // What is on disk is the restored state.
        drop(vault);
        let reopened = UnlockedVault::open(dir.path().join("v.wallet"), "pw").unwrap();
        assert_eq!(reopened.doc.stores.len(), with_both);
    }

    #[test]
    fn a_restore_outside_the_backup_folder_changes_nothing() {
        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let mut vault = vault_with_store(dir.path().join("v.wallet"), "Mine");
        let foreign = outside.path().join("v.wallet.evil.bak");
        std::fs::copy(&vault.path, &foreign).unwrap();
        let before = std::fs::read(&vault.path).unwrap();
        let listed = vault.list_backups().unwrap().len();

        for name in [
            foreign.to_string_lossy().as_ref(),
            "../v.wallet.evil.bak",
            "v.wallet",
        ] {
            assert!(matches!(
                vault.restore_backup(name),
                Err(Error::InvalidInput(_))
            ));
        }
        assert_eq!(std::fs::read(&vault.path).unwrap(), before);
        assert_eq!(vault.list_backups().unwrap().len(), listed);
    }

    #[test]
    fn a_backup_under_another_password_is_refused_untouched() {
        let dir = tempdir().unwrap();
        let mut vault = vault_with_store(dir.path().join("v.wallet"), "Mine");
        let old = vault.create_backup().unwrap();
        vault.change_password("pw", "new").unwrap();
        let before = std::fs::read(&vault.path).unwrap();
        assert!(matches!(
            vault.restore_backup(&old.name),
            Err(Error::Secret)
        ));
        assert_eq!(std::fs::read(&vault.path).unwrap(), before);
        // No safety copy was left behind by the refused restore.
        assert_eq!(vault.list_backups().unwrap().len(), 1);
    }
}
