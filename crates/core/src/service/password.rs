//! A vault with or without a password: ask, set, change, remove, and what
//! happens to the copies of the file and to the device secrets meanwhile.
//!
//! "Without a password" is a property of the file (see `crypto`): its header
//! says so and the payload is not encrypted. Setting, changing or removing
//! the password rewrites the file at once, atomically.
//!
//! A new key ([`UnlockedVault::set_password_with`],
//! [`UnlockedVault::change_password_with`]) goes through these steps, each of
//! which leaves files one of the two passwords opens if the process dies
//! right after it:
//!
//! 1. the device secrets are sealed under the new key into `.secrets.next`
//!    (the file under the old key stays);
//! 2. the vault is written under the new key (atomic rename; the mirror gets
//!    the same bytes in the same write) and read back from the disk with it;
//! 3. `.secrets.next` replaces `.secrets` (a load with the new key does the
//!    same if the process died before);
//! 4. the copies: each is re-sealed into a temp file that is read back and
//!    opened before it is renamed over the copy; a plain copy is deleted only
//!    on request and only after step 2 proved the new file opens.

use std::sync::Arc;

use zeroize::Zeroizing;

use super::UnlockedVault;
use super::security_level;
use crate::crypto::{self, SealKey, device_secrets};
use crate::error::{Error, Result};
use crate::ipc_contract::security::SecurityLevel;
use crate::storage::{SnapshotFormat, VaultFile, decode_versioned};
use crate::vault_io::backups;
use crate::vault_io::copies::{self, CopyKind, CopySeal, VaultCopy};

/// What a new password does with the vault's copies that are not encrypted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlainCopies {
    /// Leave them; they are reported for the user to choose.
    #[default]
    Keep,
    /// Seal them under the new key.
    Reencrypt,
    /// Delete them (after the vault under the new key opened from the disk).
    Delete,
}

/// The caller's choice for the copies when the key changes. The copies from
/// before a sync always follow the vault's key: the restore button opens
/// them with the session's password.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CopyPolicy {
    pub plain: PlainCopies,
    /// Also seal under the new key the copies the old password opens
    /// (`change_password` only: `set_password` has no old one).
    pub reencrypt_old: bool,
}

/// What a password change did with the copies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RotationReport {
    /// Copies now sealed under the new key.
    pub reencrypted: Vec<String>,
    /// Copies deleted.
    pub deleted: Vec<String>,
    /// Copies the new password does not open: plain, under another key
    /// (the old password among them), unreadable. For the user's choice.
    pub left: Vec<VaultCopy>,
    /// The new file went to the external copy in the same write.
    pub mirrored: bool,
}

/// The points of a key change a test can break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RotateStep {
    SecretsStaged,
    VaultWritten,
    SecretsFinished,
    CopyDone,
}

impl UnlockedVault {
    /// Whether the file is encrypted (the vault has a password).
    pub fn is_encrypted(&self) -> bool {
        self.key.as_ref().is_none_or(|key| !key.is_plain())
    }

    /// The vault's copies next to it and the key each is sealed under,
    /// relative to this session's key (no password asked, no KDF run).
    pub fn list_vault_copies(&self) -> Result<Vec<VaultCopy>> {
        copies::inventory(&self.path, &*self.session_key()?)
    }

    /// Give a vault without a password one. The file is rewritten encrypted
    /// first; only then, if `delete_plain_copies`, the vault's backups made
    /// without a password are deleted. Returns how many were deleted.
    pub fn set_password(&mut self, new_password: &str, delete_plain_copies: bool) -> Result<u32> {
        let plain = if delete_plain_copies {
            PlainCopies::Delete
        } else {
            PlainCopies::Keep
        };
        let report = self.set_password_with(
            new_password,
            CopyPolicy {
                plain,
                reencrypt_old: false,
            },
        )?;
        Ok(u32::try_from(report.deleted.len()).unwrap_or(u32::MAX))
    }

    /// [`Self::set_password`] with a choice for every plain copy. The new key
    /// is at the standard cost (a vault without a password is level 1).
    pub fn set_password_with(
        &mut self,
        new_password: &str,
        policy: CopyPolicy,
    ) -> Result<RotationReport> {
        self.set_password_inner(new_password, policy, |_| Ok(()))
    }

    fn set_password_inner(
        &mut self,
        new_password: &str,
        policy: CopyPolicy,
        fault: impl FnMut(RotateStep) -> Result<()>,
    ) -> Result<RotationReport> {
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
        // A password at the standard cost is level 2.
        self.check_not_below_min(SecurityLevel::Pin)?;
        let key = SealKey::fresh(new_password.as_bytes())?;
        self.rotate(key, new_password, None, policy, fault)
    }

    /// [`Self::change_password`] with a choice for the copies: the plain ones
    /// and the ones the old password opens. The current password is checked
    /// against the file first (a wrong one is `SECRET` and nothing changes);
    /// the new key keeps the vault's KDF cost (a level-3 vault stays level 3).
    pub fn change_password_with(
        &mut self,
        current_password: &str,
        new_password: &str,
        policy: CopyPolicy,
    ) -> Result<RotationReport> {
        self.change_password_inner(current_password, new_password, policy, |_| Ok(()))
    }

    pub(super) fn change_password_inner(
        &mut self,
        current_password: &str,
        new_password: &str,
        policy: CopyPolicy,
        fault: impl FnMut(RotateStep) -> Result<()>,
    ) -> Result<RotationReport> {
        if self.sandbox {
            return Err(Error::Unsupported);
        }
        if new_password.is_empty() {
            return Err(Error::InvalidInput(
                "the new password must not be empty".into(),
            ));
        }
        if !self.is_encrypted() {
            return Err(Error::InvalidInput(
                "the vault has no password to change: set one".into(),
            ));
        }
        // At level 3 the new password meets the level-3 rule: a short one
        // would take the vault's protection down while its cost stays.
        if self.security_level() == SecurityLevel::Crypto {
            security_level::check_new_secret(SecurityLevel::Crypto, new_password)?;
        }
        VaultFile::open(&self.path, current_password)?;
        // A new salt at the same cost: `SealKey::fresh` would quietly take a
        // hardened vault down to the standard cost.
        let key = self
            .session_key()?
            .with_new_password(new_password.as_bytes())?;
        self.rotate(key, new_password, Some(current_password), policy, fault)
    }

    /// Take the password off. The current password is checked against the
    /// file on disk first (a wrong one is `SECRET` and nothing changes); the
    /// file is then rewritten without encryption. The pre-sync copies are
    /// deleted (they are sealed under the password just removed and nothing
    /// could open them any more); other backups stay as they are, encrypted
    /// under the password they were made with.
    ///
    /// Refused (`SECURITY_DOWNGRADE_REFUSED`) while the vault holds
    /// crypto-module data: exchange keys on this device would end up under a
    /// key anyone can derive.
    pub fn remove_password(&mut self, current_password: &str) -> Result<()> {
        if self.sandbox {
            return Err(Error::Unsupported);
        }
        if !self.is_encrypted() {
            return Err(Error::InvalidInput("the vault has no password".into()));
        }
        self.check_not_below_min(SecurityLevel::Ledger)?;
        VaultFile::open(&self.path, current_password)?;
        self.rekey(Arc::new(SealKey::plain()), "")?;
        // The pre-sync copies are sealed under the password just removed: the
        // restore button would offer a copy nothing can open.
        self.remove_sync_backups();
        Ok(())
    }

    pub(super) fn session_key(&self) -> Result<Arc<SealKey>> {
        self.key
            .clone()
            .ok_or_else(|| Error::Crypto("the vault has no key".into()))
    }

    /// Switch to `key` and write the file under it now; on a failed write the
    /// old key and password stay, matching the unchanged file.
    fn rekey(&mut self, key: Arc<SealKey>, password: &str) -> Result<()> {
        let old_key = self.key.replace(key);
        let old_password = std::mem::replace(&mut self.password, Zeroizing::new(password.into()));
        if let Err(err) = self.persist_now() {
            self.key = old_key;
            self.password = old_password;
            return Err(err);
        }
        Ok(())
    }

    /// The vault file as it is on the disk opens with `key` and holds this
    /// vault: read back, no KDF (the key's salt and cost must be the
    /// header's, so the password that made the key opens it too).
    pub(super) fn verify_on_disk(&self, key: &SealKey) -> Result<()> {
        let bytes = Zeroizing::new(crypto::read_all(&self.path)?);
        let (payload, fmt) = crypto::open_bytes_with(&bytes, key)?;
        let decoded = decode_versioned(&Zeroizing::new(payload), fmt)?;
        if decoded.doc.id != self.doc.id {
            return Err(Error::Crypto(
                "the vault written under the new key is not this vault".into(),
            ));
        }
        Ok(())
    }

    /// Move the vault, its device secrets and its copies onto `new_key` (see
    /// the module docs for the steps). `old_password` opens copies under
    /// another salt of the old password (change only).
    fn rotate(
        &mut self,
        new_key: SealKey,
        new_password: &str,
        old_password: Option<&str>,
        policy: CopyPolicy,
        mut fault: impl FnMut(RotateStep) -> Result<()>,
    ) -> Result<RotationReport> {
        self.flush_pending()?;
        let old_key = self.session_key()?;
        let new_key = Arc::new(new_key);
        // Looked at before anything is written: "current key" means the old one.
        let found = copies::inventory(&self.path, &old_key)?;

        // A secrets file the old key does not open is damaged: it was of no
        // use before and stays as it is (the next stored key replaces it).
        let staged = match device_secrets::stage_rekey(&self.path, &old_key, &new_key) {
            Ok(()) => true,
            Err(Error::Secret) => false,
            Err(err) => return Err(err),
        };
        fault(RotateStep::SecretsStaged)?;

        self.rekey(new_key.clone(), new_password)?;
        fault(RotateStep::VaultWritten)?;
        self.verify_on_disk(&new_key)?;

        if staged {
            device_secrets::finish_rekey(&self.path)?;
        }
        fault(RotateStep::SecretsFinished)?;

        let mut report = RotationReport {
            mirrored: self.has_mirror(),
            ..RotationReport::default()
        };
        let mut old_keys: Vec<SealKey> = Vec::new();
        for copy in found {
            self.rotate_copy(
                &copy,
                &old_key,
                old_password,
                &mut old_keys,
                &new_key,
                policy,
                &mut report,
            )?;
            fault(RotateStep::CopyDone)?;
        }
        report.left = copies::inventory(&self.path, &new_key)?
            .into_iter()
            .filter(|copy| copy.seal != CopySeal::CurrentKey)
            .collect();
        Ok(report)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn rotate_copy(
        &self,
        copy: &VaultCopy,
        old_key: &SealKey,
        old_password: Option<&str>,
        old_keys: &mut Vec<SealKey>,
        new_key: &SealKey,
        policy: CopyPolicy,
        report: &mut RotationReport,
    ) -> Result<()> {
        let name = copy.file.name.as_str();
        let presync = copy.kind == CopyKind::PreSync;
        let wanted = presync
            || match copy.seal {
                CopySeal::Plain => policy.plain == PlainCopies::Reencrypt,
                CopySeal::CurrentKey | CopySeal::OtherKey => policy.reencrypt_old,
                CopySeal::Unreadable => false,
            };
        if !presync && copy.seal == CopySeal::Plain && policy.plain == PlainCopies::Delete {
            // Step 2 proved the vault under the new key opens from the disk.
            std::fs::remove_file(backups::resolve(&self.path, name)?)?;
            report.deleted.push(name.to_string());
            return Ok(());
        }
        if !wanted {
            return Ok(());
        }
        let resealed = match copy.seal {
            CopySeal::Plain => copies::reseal(
                &self.path,
                name,
                |bytes| crypto::open_bytes_keyed(bytes, b"").map(|(plain, fmt, _)| (plain, fmt)),
                new_key,
            ),
            CopySeal::CurrentKey => copies::reseal(
                &self.path,
                name,
                |bytes| crypto::open_bytes_with(bytes, old_key),
                new_key,
            ),
            CopySeal::OtherKey => match old_password {
                Some(password) => copies::reseal(
                    &self.path,
                    name,
                    |bytes| open_with_old_password(bytes, password, old_keys),
                    new_key,
                ),
                None => Err(Error::Secret),
            },
            CopySeal::Unreadable => Err(Error::Secret),
        };
        match resealed {
            Ok(()) => report.reencrypted.push(name.to_string()),
            // A copy from before a sync that the old password does not open
            // either: the restore button could never use it (a password
            // change has always removed these).
            Err(_) if presync && old_password.is_some() => {
                std::fs::remove_file(backups::resolve(&self.path, name)?)?;
                report.deleted.push(name.to_string());
            }
            // Under a password this change does not know, or a disk error:
            // the copy is as it was and the report lists it.
            Err(_) => {}
        }
        Ok(())
    }
}

/// Open a copy with the old password, deriving its key once per salt and
/// cost (copies of one password share a salt).
fn open_with_old_password(
    bytes: &[u8],
    password: &str,
    known: &mut Vec<SealKey>,
) -> Result<(Vec<u8>, SnapshotFormat)> {
    if let Some(key) = known.iter().find(|key| crypto::sealed_under(bytes, key)) {
        return crypto::open_bytes_with(bytes, key);
    }
    let (payload, fmt, key) = crypto::open_bytes_keyed(bytes, password.as_bytes())?;
    known.push(key);
    Ok((payload, fmt))
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

    // ---- a new key: copies, device secrets, mirror, crashes ----

    use crate::ipc_contract::security::SecurityLevel;
    use crate::model::store::{ExchangeProvider, StoreKind};
    use crate::service::StoreExtras;

    fn bybit(vault: &mut UnlockedVault) -> String {
        vault
            .create_store_with(
                "Bybit",
                StoreKind::Exchange,
                "#f0b90b",
                "",
                None,
                StoreExtras {
                    provider: Some(ExchangeProvider::Bybit),
                    ..Default::default()
                },
            )
            .unwrap()
            .id
    }

    /// A level-3 password the strength rule takes.
    const STRONG: &str = "Tr0ub4dor&3-Horse-Battery!";

    /// A level-3 vault (password "pw") with an exchange key on this device.
    fn level3_with_key() -> (tempfile::TempDir, UnlockedVault, String) {
        let (dir, mut vault) = empty_vault();
        vault.raise_to_level3_for_tests();
        let store = bybit(&mut vault);
        vault
            .store_device_key(&store, "KEYKEYKEY123", "SECRETSECRET456", true)
            .unwrap();
        vault.flush_pending().unwrap();
        (dir, vault, store)
    }

    fn copy_names(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|file| file.to_string_lossy().ends_with(".bak"))
            .collect();
        names.sort();
        names
    }

    fn opens(file: &std::path::Path, password: &str) -> bool {
        VaultFile::open(file, password).is_ok()
    }

    #[test]
    fn the_copies_are_listed_with_their_seal() {
        let (_dir, mut vault) = empty_vault();
        let backup = vault.create_backup().unwrap();
        vault.sync_backup().unwrap();
        let listed = vault.list_vault_copies().unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|copy| copy.seal == CopySeal::CurrentKey));
        assert!(listed.iter().any(|copy| copy.file.name == backup.name));
        assert!(listed.iter().any(|copy| copy.kind == CopyKind::PreSync));
    }

    #[test]
    fn plain_copies_are_sealed_under_the_new_password_on_request() {
        let (_dir, mut vault) = plain_vault();
        let backup = vault.create_backup().unwrap();
        let report = vault
            .set_password_with(
                "pw",
                CopyPolicy {
                    plain: PlainCopies::Reencrypt,
                    reencrypt_old: false,
                },
            )
            .unwrap();
        assert_eq!(report.reencrypted, vec![backup.name.clone()]);
        assert!(report.left.is_empty() && report.deleted.is_empty());
        let file = backups::resolve(&vault.path, &backup.name).unwrap();
        assert!(file_is_encrypted(&file));
        assert!(opens(&file, "pw"));
    }

    #[test]
    fn kept_plain_copies_are_reported() {
        let (_dir, mut vault) = plain_vault();
        let backup = vault.create_backup().unwrap();
        let report = vault
            .set_password_with("pw", CopyPolicy::default())
            .unwrap();
        assert_eq!(report.left.len(), 1);
        assert_eq!(report.left[0].file.name, backup.name);
        assert_eq!(report.left[0].seal, CopySeal::Plain);
    }

    #[test]
    fn no_plain_copy_survives_a_change_that_deletes_them() {
        let (dir, mut vault) = empty_vault();
        let plain = {
            let (other_dir, other) = plain_vault();
            let bytes = std::fs::read(&other.path).unwrap();
            drop(other);
            drop(other_dir);
            bytes
        };
        std::fs::write(dir.path().join("test.wallet.old.bak"), plain).unwrap();
        let report = vault
            .change_password_with(
                "pw",
                "new",
                CopyPolicy {
                    plain: PlainCopies::Delete,
                    reencrypt_old: false,
                },
            )
            .unwrap();
        assert_eq!(report.deleted, vec!["test.wallet.old.bak".to_string()]);
        assert!(
            copy_names(dir.path())
                .iter()
                .all(|file| file_is_encrypted(file))
        );
    }

    #[test]
    fn copies_under_the_old_password_follow_it_on_request() {
        let (dir, mut vault) = empty_vault();
        let first = vault.create_backup().unwrap();
        // The same password under a new salt: `first` is now under another key.
        vault.change_password("pw", "pw").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let second = vault.create_backup().unwrap();
        assert_ne!(first.name, second.name);
        let ancient = {
            let other_dir = tempdir().unwrap();
            let other =
                UnlockedVault::create(other_dir.path().join("a.wallet"), "ancient").unwrap();
            let bytes = std::fs::read(&other.path).unwrap();
            drop(other);
            bytes
        };
        std::fs::write(dir.path().join("test.wallet.ancient.bak"), ancient).unwrap();
        let seals: Vec<_> = vault
            .list_vault_copies()
            .unwrap()
            .into_iter()
            .map(|copy| (copy.file.name, copy.seal))
            .collect();
        assert!(seals.contains(&(first.name.clone(), CopySeal::OtherKey)));
        assert!(seals.contains(&(second.name.clone(), CopySeal::CurrentKey)));

        let report = vault
            .change_password_with(
                "pw",
                "new",
                CopyPolicy {
                    plain: PlainCopies::Keep,
                    reencrypt_old: true,
                },
            )
            .unwrap();
        for name in [&first.name, &second.name] {
            let file = backups::resolve(&vault.path, name).unwrap();
            assert!(opens(&file, "new"), "{name}");
            assert!(!opens(&file, "pw"), "{name}");
            assert!(report.reencrypted.contains(name));
        }
        assert_eq!(report.left.len(), 1);
        assert_eq!(report.left[0].file.name, "test.wallet.ancient.bak");
        assert_eq!(report.left[0].seal, CopySeal::OtherKey);
        assert!(opens(
            &dir.path().join("test.wallet.ancient.bak"),
            "ancient"
        ));
    }

    #[test]
    fn old_copies_stay_unless_asked() {
        let (_dir, mut vault) = empty_vault();
        let backup = vault.create_backup().unwrap();
        let report = vault
            .change_password_with("pw", "new", CopyPolicy::default())
            .unwrap();
        assert!(report.reencrypted.is_empty());
        assert_eq!(report.left.len(), 1);
        assert_eq!(report.left[0].seal, CopySeal::OtherKey);
        assert!(opens(
            &backups::resolve(&vault.path, &backup.name).unwrap(),
            "pw"
        ));
    }

    #[test]
    fn a_hardened_vault_keeps_its_cost_and_its_device_keys() {
        let (dir, mut vault, store) = level3_with_key();
        let before = vault.session_key().unwrap();
        vault.change_password("pw", STRONG).unwrap();
        let after = vault.session_key().unwrap();
        assert_eq!(after.kdf(), before.kdf());
        assert_ne!(after.salt(), before.salt());
        assert!(device_secrets::load(&vault.path, &before).is_err());
        let mut staged = vault.path.as_os_str().to_owned();
        staged.push(".secrets.next");
        assert!(!std::path::Path::new(&staged).exists());
        drop(vault);
        let path = dir.path().join("test.wallet");
        assert!(matches!(
            UnlockedVault::open(path.clone(), "pw"),
            Err(Error::Secret)
        ));
        let reopened = UnlockedVault::open(path, STRONG).unwrap();
        assert_eq!(reopened.security_level(), SecurityLevel::Crypto);
        let key = reopened.device_key(&store).unwrap().expect("the key stays");
        assert_eq!(key.keys.api_key, "KEYKEYKEY123");
    }

    #[test]
    fn the_password_does_not_come_off_while_keys_are_on_the_device() {
        let (_dir, mut vault, _store) = level3_with_key();
        let before = std::fs::read(&vault.path).unwrap();
        assert!(matches!(
            vault.remove_password("pw"),
            Err(Error::SecurityDowngradeRefused)
        ));
        assert!(vault.is_encrypted());
        assert_eq!(std::fs::read(&vault.path).unwrap(), before);
    }

    #[derive(Default)]
    struct RecordingMirror(std::sync::Mutex<Vec<Vec<u8>>>);

    impl crate::vault_io::VaultMirror for RecordingMirror {
        fn push(&self, sealed: &[u8]) {
            self.0.lock().unwrap().push(sealed.to_vec());
        }
    }

    #[test]
    fn the_mirror_gets_the_file_under_the_new_key_at_once() {
        let (_dir, mut vault) = empty_vault();
        let mirror = Arc::new(RecordingMirror::default());
        vault.set_mirror(Some(mirror.clone()));
        let report = vault
            .change_password_with("pw", "new", CopyPolicy::default())
            .unwrap();
        assert!(report.mirrored);
        let on_disk = std::fs::read(&vault.path).unwrap();
        assert_eq!(mirror.0.lock().unwrap().last(), Some(&on_disk));

        let (_plain_dir, mut plain) = plain_vault();
        let mirror = Arc::new(RecordingMirror::default());
        plain.set_mirror(Some(mirror.clone()));
        plain.set_password("pw", false).unwrap();
        let on_disk = std::fs::read(&plain.path).unwrap();
        assert!(file_is_encrypted(&plain.path));
        assert_eq!(mirror.0.lock().unwrap().last(), Some(&on_disk));
    }

    /// Kill the change at `step`; whatever is on the disk then must open with
    /// one of the two passwords, the keys included.
    fn crash_at(step: RotateStep) {
        let (dir, mut vault, store) = level3_with_key();
        vault.sync_backup().unwrap();
        vault.create_backup().unwrap();
        let path = vault.path.clone();
        let result = vault.change_password_inner(
            "pw",
            "new",
            CopyPolicy {
                plain: PlainCopies::Keep,
                reencrypt_old: true,
            },
            |at| {
                if at == step {
                    Err(Error::Io("injected".into()))
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err(), "{step:?}");
        drop(vault);

        let reopened = UnlockedVault::open(path.clone(), "pw")
            .or_else(|_| UnlockedVault::open(path.clone(), "new"))
            .unwrap_or_else(|err| panic!("{step:?}: the vault opens with neither: {err}"));
        let key = reopened
            .device_key(&store)
            .unwrap_or_else(|err| panic!("{step:?}: keys unreadable: {err}"));
        assert!(key.is_some(), "{step:?}: the device key is lost");
        drop(reopened);
        for copy in copy_names(dir.path()) {
            assert!(
                opens(&copy, "pw") || opens(&copy, "new"),
                "{step:?}: {copy:?} opens with neither"
            );
        }
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{step:?}: {leftovers:?}");
    }

    #[test]
    fn a_change_killed_at_any_step_leaves_files_that_open() {
        for step in [
            RotateStep::SecretsStaged,
            RotateStep::VaultWritten,
            RotateStep::SecretsFinished,
            RotateStep::CopyDone,
        ] {
            crash_at(step);
        }
    }
}
