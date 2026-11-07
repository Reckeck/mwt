//! Security levels (H5A): the strength rules for a new secret, the lowest
//! level the vault may go to, the raise to the crypto level, and the level
//! check on what a sync brings in.
//!
//! The raise to level 3 ([`UnlockedVault::raise_to_crypto`]) leaves, if the
//! process dies after any step, a vault that opens at exactly one level with
//! its data whole:
//!
//! 1. a backup of the file as it is (level 2 opens with the old PIN, level 1
//!    is a plain copy); it stays after an interruption, the user deletes it;
//! 2. the file is re-encrypted at the hardened cost into a temp file, read
//!    back, opened and compared, and only then renamed over the vault
//!    (`VaultFile::reencrypt_verified`); before the rename the vault is at
//!    its old level (1 or 2), after it level 3;
//! 3. the session takes the new key; the device secrets follow it and the
//!    keys an old document still holds move to the device file;
//! 4. the document is written again under the new key (the external copy
//!    gets those bytes in the same write) and read back from the disk;
//! 5. the copies next to the vault cheaper than level 3 (the backup of step
//!    1, `.bak`, `.vN.bak`, the pre-sync ones) are re-sealed under the new
//!    key, each through a temp file that is opened before the rename; what
//!    cannot be re-sealed and is still cheaper is deleted, only now that step
//!    4 proved the new vault opens from the disk.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zeroize::Zeroizing;

use super::UnlockedVault;
use super::password::{CopyPolicy, PlainCopies, RotationReport};
use crate::crypto::header::{VaultHeader, level_of_container};
use crate::crypto::kdf_profile::{KdfParams, KdfProfile};
use crate::crypto::{SealKey, device_secrets};
use crate::error::{Error, Result};
use crate::ipc_contract::security::{
    PasswordStrength, PasswordStrengthReport, RaiseStage, SecurityLevel,
};
use crate::model::vault::VaultDoc;
use crate::storage::VaultFile;
use crate::sync::state::RecordState;
use crate::vault_io::backups;
use crate::vault_io::copies::{self, CopySeal};

/// A new level-2 PIN: at least this many digits, digits only.
pub const MIN_PIN_DIGITS: usize = 6;
/// A level-3 password: at least this many characters (and not weak).
pub const MIN_CRYPTO_PASSWORD: usize = 16;

/// Whether `pin` may be set as a new level-2 PIN. An old shorter password
/// still opens its vault and is accepted where the current one is typed.
pub fn pin_ok(pin: &str) -> bool {
    pin.len() >= MIN_PIN_DIGITS && pin.bytes().all(|byte| byte.is_ascii_digit())
}

/// How strong `password` is for the crypto level, and whether it is long
/// enough. The same rule is in `src/shared/lib/vault-password.ts`.
pub fn password_strength(password: &str) -> PasswordStrengthReport {
    let chars: Vec<char> = password.chars().collect();
    PasswordStrengthReport {
        strength: grade(&chars),
        long_enough: chars.len() >= MIN_CRYPTO_PASSWORD,
    }
}

/// A simple own heuristic, no dictionary: short, few distinct characters or
/// mostly runs (`aaaa`, `abcd`, `1234`) is weak; long with several kinds of
/// characters is strong; the rest is medium.
fn grade(chars: &[char]) -> PasswordStrength {
    let count = chars.len();
    if count < MIN_CRYPTO_PASSWORD {
        return PasswordStrength::Weak;
    }
    let distinct = chars.iter().collect::<BTreeSet<_>>().len();
    let runs = chars
        .windows(2)
        .filter(|pair| (i64::from(u32::from(pair[0])) - i64::from(u32::from(pair[1]))).abs() <= 1)
        .count();
    if distinct < 8 || runs * 2 >= count - 1 {
        return PasswordStrength::Weak;
    }
    let lower = chars.iter().any(|ch| ch.is_lowercase());
    let upper = chars.iter().any(|ch| ch.is_uppercase());
    let digit = chars.iter().any(char::is_ascii_digit);
    let other = chars
        .iter()
        .any(|ch| !ch.is_lowercase() && !ch.is_uppercase() && !ch.is_ascii_digit());
    let classes = [lower, upper, digit, other]
        .into_iter()
        .filter(|has| *has)
        .count();
    if (count >= 20 && classes >= 3 && distinct >= 12) || (count >= 24 && distinct >= 10) {
        PasswordStrength::Strong
    } else {
        PasswordStrength::Medium
    }
}

/// Whether `password` may protect a level-3 vault.
pub fn crypto_password_ok(password: &str) -> bool {
    let report = password_strength(password);
    report.long_enough && report.strength != PasswordStrength::Weak
}

/// The rule for a new secret of a vault at `level`: none for level 1, a PIN
/// of six digits or more for level 2, a long and not weak password for 3.
pub fn check_new_secret(level: SecurityLevel, secret: &str) -> Result<()> {
    match level {
        SecurityLevel::Ledger => Ok(()),
        SecurityLevel::Pin if pin_ok(secret) => Ok(()),
        SecurityLevel::Pin => Err(Error::PinFormat),
        SecurityLevel::Crypto if crypto_password_ok(secret) => Ok(()),
        SecurityLevel::Crypto => Err(Error::PasswordTooWeak),
    }
}

/// Order of the levels, cheapest first.
pub fn level_rank(level: SecurityLevel) -> u8 {
    match level {
        SecurityLevel::Ledger => 0,
        SecurityLevel::Pin => 1,
        SecurityLevel::Crypto => 2,
    }
}

/// The level a vault file's header claims, read from its first bytes.
pub fn level_of_vault_file(path: &Path) -> Result<SecurityLevel> {
    let mut head = vec![0u8; crate::VAULT_MAGIC.len() + VaultHeader::LEN];
    let mut file = std::fs::File::open(path)?;
    file.read_exact(&mut head)?;
    level_of_container(&head)
}

/// Whether the vault bytes `remote` are protected more cheaply than `local`.
/// Bytes whose header cannot be read are not judged here (the import check
/// refuses them anyway).
pub fn container_is_cheaper(local: &[u8], remote: &[u8]) -> bool {
    match (level_of_container(local), level_of_container(remote)) {
        (Ok(mine), Ok(theirs)) => level_rank(theirs) < level_rank(mine),
        _ => false,
    }
}

/// Whether the document holds crypto-module data, which ties it to level 3.
/// Stage 1 has none (`VaultDoc` has no crypto section yet); E0 extends this.
pub fn doc_carries_crypto_data(_doc: &VaultDoc) -> bool {
    false
}

/// What the raise to level 3 did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RaiseReport {
    /// The backup made first (re-sealed under the new key at the end).
    pub backup: PathBuf,
    /// Copies now sealed under the new key.
    pub resealed: Vec<String>,
    /// Copies cheaper than level 3 that could not be re-sealed, deleted.
    pub deleted: Vec<String>,
    /// The new file went to the external copy.
    pub mirrored: bool,
}

/// The points of the raise a test can break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RaiseStep {
    BackedUp,
    Reencrypted,
    Adopted,
    Persisted,
    CopyDone,
}

impl UnlockedVault {
    /// The lowest level this vault may be protected at: level 3 once it holds
    /// crypto-module data (stage 1: exchange keys on this device, or their
    /// file next to the vault), else level 1.
    pub fn min_allowed_level(&self) -> SecurityLevel {
        let secrets_file = !self.sandbox && device_secrets::secrets_path(&self.path).exists();
        if self.has_device_keys() || secrets_file || doc_carries_crypto_data(&self.doc) {
            SecurityLevel::Crypto
        } else {
            SecurityLevel::Ledger
        }
    }

    /// Refuse a change that would leave the vault below its lowest level.
    pub(super) fn check_not_below_min(&self, target: SecurityLevel) -> Result<()> {
        if level_rank(target) < level_rank(self.min_allowed_level()) {
            return Err(Error::SecurityDowngradeRefused);
        }
        Ok(())
    }

    /// The device profile the raise calibrates against: a vault with an
    /// external copy may be opened on a phone, so it takes the phone's (at
    /// most 256 MiB); otherwise this device's.
    pub fn raise_profile(&self) -> KdfProfile {
        if self.has_mirror() {
            KdfProfile::MOBILE
        } else {
            KdfProfile::for_this_device()
        }
    }

    /// Refuse a sync batch with crypto-module data unless this vault is at
    /// level 3 (`SECURITY_LEVEL_REQUIRED`).
    pub fn sync_check_level(
        &self,
        incoming: &std::collections::BTreeMap<String, RecordState>,
    ) -> Result<()> {
        if self.security_level() != SecurityLevel::Crypto
            && crate::sync::crypto_gate::carries_crypto_data(incoming.keys())
        {
            return Err(Error::SecurityLevelRequired);
        }
        Ok(())
    }

    /// Raise the vault to level 3 with `new_password` at the cost `kdf` (see
    /// the module docs for the steps). `current_password` is checked first; a
    /// wrong one is `SECRET` and nothing changes. A plain vault (level 1) goes
    /// straight to level 3 and its current password is ignored. `on_stage` hears each stage
    /// before it starts.
    pub fn raise_to_crypto(
        &mut self,
        current_password: &str,
        new_password: &str,
        kdf: KdfParams,
        on_stage: impl FnMut(RaiseStage),
    ) -> Result<RaiseReport> {
        if !crypto_password_ok(new_password) {
            return Err(Error::PasswordTooWeak);
        }
        self.raise_inner(current_password, new_password, kdf, on_stage, |_| Ok(()))
    }

    pub(super) fn raise_inner(
        &mut self,
        current_password: &str,
        new_password: &str,
        kdf: KdfParams,
        mut on_stage: impl FnMut(RaiseStage),
        mut fault: impl FnMut(RaiseStep) -> Result<()>,
    ) -> Result<RaiseReport> {
        if self.sandbox {
            return Err(Error::Unsupported);
        }
        if self.security_level() == SecurityLevel::Crypto {
            return Err(Error::InvalidInput(
                "the vault is already at level 3".into(),
            ));
        }
        if kdf.id != VaultHeader::KDF_ARGON2ID_HARDENED {
            return Err(Error::InvalidInput(
                "level 3 needs the hardened key cost".into(),
            ));
        }
        kdf.check()?;
        // A plain vault (level 1) has no current secret: nothing to check, and
        // its plain copies are re-sealed below like the level-2 ones.
        self.check_password(current_password)?;
        self.flush_pending()?;

        on_stage(RaiseStage::Backup);
        let backup = self.create_backup()?;
        fault(RaiseStep::BackedUp)?;

        let old_key = self.session_key()?;
        let old_password = self.password.clone();
        // Looked at before the vault changes: "current key" is the old one,
        // and the backup just made is among them.
        let found = copies::inventory(&self.path, &old_key)?;

        let new_key = Arc::new(VaultFile::reencrypt_verified(
            &self.path,
            &self.doc,
            self.fmt,
            new_password,
            kdf,
            &mut on_stage,
        )?);
        // The file is level 3 from here: the session follows it at once, so
        // a later failure never writes cheaper bytes over it.
        self.key = Some(new_key.clone());
        self.password = Zeroizing::new(new_password.into());
        fault(RaiseStep::Reencrypted)?;

        // Below level 3 the device file is not used; one left from earlier
        // that the old key opens follows the new key, any other stays.
        match device_secrets::rekey_device_secrets(&self.path, &old_key, &new_key) {
            Ok(()) | Err(Error::Secret) => {}
            Err(err) => return Err(err),
        }
        self.adopt_device_secrets()?;
        fault(RaiseStep::Adopted)?;

        // Without the keys the old document held, and to the external copy.
        self.persist_now()?;
        self.verify_on_disk(&new_key)?;
        fault(RaiseStep::Persisted)?;

        let old_password = (!old_password.is_empty()).then_some(old_password);
        let mut rotation = RotationReport {
            mirrored: self.has_mirror(),
            ..RotationReport::default()
        };
        let policy = CopyPolicy {
            plain: PlainCopies::Reencrypt,
            reencrypt_old: true,
        };
        let mut old_keys: Vec<SealKey> = Vec::new();
        for copy in &found {
            self.rotate_copy(
                copy,
                &old_key,
                old_password.as_deref().map(String::as_str),
                &mut old_keys,
                &new_key,
                policy,
                &mut rotation,
            )?;
            fault(RaiseStep::CopyDone)?;
        }
        // Whatever is still cheaper than level 3 goes: the new vault opened
        // from the disk above. A file whose header says nothing is left.
        for copy in copies::inventory(&self.path, &new_key)? {
            if copy.seal == CopySeal::CurrentKey {
                continue;
            }
            let file = backups::resolve(&self.path, &copy.file.name)?;
            if level_of_vault_file(&file).is_ok_and(|level| level != SecurityLevel::Crypto) {
                std::fs::remove_file(&file)?;
                rotation.deleted.push(copy.file.name.clone());
            }
        }
        self.after_raise();
        Ok(RaiseReport {
            backup: backups::resolve(&self.path, &backup.name)?,
            resealed: rotation.reencrypted,
            deleted: rotation.deleted,
            mirrored: rotation.mirrored,
        })
    }

    /// What follows a successful raise. Empty in stage 1 (E11 hooks in here).
    fn after_raise(&mut self) {}
}

#[cfg(test)]
impl UnlockedVault {
    /// The vault raised to level 3 in place through the real path, keeping
    /// its password, at the cheapest hardened cost; the wizard's backup is
    /// removed so the tests see the vault's own copies only.
    pub(crate) fn raise_to_level3_for_tests(&mut self) {
        use crate::crypto::kdf_profile::KDF_MEM_KIB_MIN;
        let password = self.password.clone();
        let kdf = KdfParams::hardened(KDF_MEM_KIB_MIN, 1, 1);
        let report = self
            .raise_inner(&password, &password, kdf, |_| {}, |_| Ok(()))
            .expect("raise");
        std::fs::remove_file(report.backup).expect("backup");
    }
}

#[cfg(test)]
#[path = "security_level_tests.rs"]
mod tests;
