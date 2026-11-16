//! Wiping the module out of the vault (spec E12).
//!
//! # The order, and why it is that order
//!
//! A wipe is the one operation of the module that cannot be taken back, so
//! the order is the whole design:
//!
//! 1. the word the user typed is checked — nothing else happens without it;
//! 2. there is a **backup**: the vault file as it is now, copied through the
//!    same [`UnlockedVault::create_backup`] the level-3 raise uses, then
//!    **opened again** with this session's password and read back. A copy
//!    that does not decrypt, or whose document no longer holds what this one
//!    holds, ends the wipe right there, with nothing touched;
//! 3. only now is anything destroyed: every `crypto_*` collection is
//!    cleared, the wallets the module led lose their link and are archived,
//!    the exchange key index goes, the capital is recounted and the mark is
//!    left behind;
//! 4. the document is written;
//! 5. the device secrets file is removed as a whole — the exchange keys, the
//!    per-network endpoints and the Blockscout key of this device go with it.
//!    It is removed after the write, because a crash between the two would
//!    otherwise leave a vault that still believes it has keys.
//!
//! The biometric binding is dropped by the command layer afterwards: the OS
//! keeps it, not the vault (`crates/app/src/commands/cryptotrack/wipe.rs`).
//!
//! # What a wipe does to a wallet, and why it is not a deletion
//!
//! A followed address is a real wallet: a `Store` of kind `Crypto` carrying
//! [`crate::model::store::Store::crypto_account_id`] (decision P5 of
//! `U2-decisions.md`). The wipe **strips that link and archives the wallet**;
//! it deletes no `Store`. A wallet carries history — operations, holdings,
//! favourites, the capital structure all point at its id — so deleting one
//! would take records with it that the module never owned. Archiving is the
//! same answer the move *into* the module gives for the wallets kept by hand
//! (`super::transition`), and the report names every wallet it archived.
//!
//! Afterwards
//! [`crate::service::security_level::doc_carries_crypto_data`] is false and
//! no wallet in the tree is led by the engine, which is what lifts the ban on
//! going below level 3.
//!
//! # What the other devices do with it
//!
//! The wipe leaves a [`CryptoWipe`] record in `crypto_wiped_at`, which syncs
//! like any other record. A device that was offline during the wipe still
//! holds the module's data and would hand it straight back; the receiving
//! side drops what the mark has outlived
//! (`crate::sync::crypto_gate::without_wiped`), and keeps whatever that
//! device made after the wipe.
//!
//! The mark is not a remote wipe. It stops the data coming back; it does not
//! empty the other device, which keeps what it already had until it is wiped
//! there too. A wipe destroys only where it was asked for, because only there
//! can it take the backup it stands on, and only there can the user be asked
//! for the word.

use crate::Amount;
use crate::crypto::device_secrets;
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::wipe::{
    CryptoWipeCounts, CryptoWipePreview, CryptoWipeReport, CryptoWipedWallet,
};
use crate::ipc_contract::security::SecurityLevel;
use crate::model::cryptotrack::CryptoWipe;
use crate::model::vault::now_iso;
use crate::service::security_level::level_rank;
use crate::storage::VaultFile;
use crate::vault_io::backups;

use super::super::UnlockedVault;

/// The token the vault accepts as "yes, wipe it".
///
/// The window asks the user to type a word in their own language and sends
/// this one once the word matches, so neither a mis-click nor a call with an
/// empty argument can wipe anything.
pub const WIPE_CONFIRM: &str = "WIPE";

impl UnlockedVault {
    /// What a wipe would remove: the records per collection, the wallets that
    /// would be unlinked and archived, what the capital would lose and which
    /// backup the user has already.
    pub fn crypto_wipe_preview(&self) -> CryptoWipePreview {
        let counts = self.crypto_counts();
        let wallets = self.crypto_engine_wallets();
        let (base_currency, _) = self.base_and_fx();
        let device_secrets = self.device_secrets_present();
        let empty = counts.is_empty() && wallets.is_empty() && !device_secrets;
        CryptoWipePreview {
            counts,
            wallets,
            base_currency,
            value_delta: self.crypto_wipe_value_delta(),
            backup_at: self.newest_backup_at(),
            device_secrets,
            empty,
        }
    }

    /// Wipe the module. See the module docs for the order; `confirm` must be
    /// [`WIPE_CONFIRM`].
    ///
    /// Nothing is removed while the backup cannot be taken and read back, and
    /// a vault that holds nothing of the module takes no backup at all: there
    /// is nothing to risk and nothing to mark.
    pub fn crypto_wipe_module(&mut self, confirm: &str) -> Result<CryptoWipeReport> {
        self.wipe_inner(confirm, Self::backup_before_wipe)
    }

    /// [`Self::crypto_wipe_module`] with the backup step handed in, so a test
    /// can watch what a failed backup leaves behind: the answer must be
    /// "everything, untouched".
    fn wipe_inner(
        &mut self,
        confirm: &str,
        backup: impl FnOnce(&mut Self) -> Result<String>,
    ) -> Result<CryptoWipeReport> {
        if confirm != WIPE_CONFIRM {
            return Err(Error::InvalidInput(
                "the wipe of the crypto module was not confirmed".into(),
            ));
        }
        if self.sandbox {
            return Err(Error::Unsupported);
        }
        let preview = self.crypto_wipe_preview();
        if preview.empty {
            return Ok(CryptoWipeReport {
                base_currency: preview.base_currency,
                can_lower_level: level_rank(self.min_allowed_level())
                    < level_rank(SecurityLevel::Crypto),
                ..CryptoWipeReport::default()
            });
        }

        let backup = backup(self)?;

        let counts = self.wipe_crypto_document();
        let wiped_at = now_iso();
        self.doc.crypto_wiped_at.push(CryptoWipe {
            id: uuid::Uuid::new_v4().to_string(),
            at: wiped_at.clone(),
            device_id: self.clock.device().to_string(),
        });
        self.persist_now()?;

        // Only now, with the emptied document on the disk: the keys, the
        // endpoints and the Blockscout key of this device go as one file.
        device_secrets::remove(&self.path)?;

        Ok(CryptoWipeReport {
            counts,
            wallets: preview.wallets,
            base_currency: preview.base_currency,
            value_delta: preview.value_delta,
            backup: Some(backup),
            device_secrets_removed: preview.device_secrets,
            wiped_at: Some(wiped_at),
            can_lower_level: level_rank(self.min_allowed_level())
                < level_rank(SecurityLevel::Crypto),
        })
    }

    /// The backup the wipe stands on: the file as it is now, copied and then
    /// opened again with this session's password and compared with what is in
    /// memory. Returns its file name.
    ///
    /// A copy that cannot be opened, or that does not hold the module's data,
    /// is no backup at all: the wipe stops and the document is still whole.
    fn backup_before_wipe(&mut self) -> Result<String> {
        let backup = self.create_backup()?;
        let path = backups::resolve(&self.path, &backup.name)?;
        let (saved, _) = VaultFile::open(&path, &self.password)?;
        if self.crypto_counts_of(&saved) != self.crypto_counts() {
            return Err(Error::Crypto(
                "the backup does not hold the crypto module: nothing was wiped".into(),
            ));
        }
        Ok(backup.name)
    }

    /// Everything the wipe does to the document itself, and nothing else: no
    /// file is touched and no mark is left, so the preview can run it on a
    /// copy in memory to see what the capital becomes.
    fn wipe_crypto_document(&mut self) -> CryptoWipeCounts {
        let counts = self.crypto_counts();
        self.doc.crypto_accounts.clear();
        self.doc.crypto_events.clear();
        self.doc.crypto_overrides.clear();
        self.doc.crypto_tokens.clear();
        self.doc.crypto_positions.clear();
        self.doc.crypto_snapshots.clear();
        self.doc.crypto_cursors.clear();
        self.doc.crypto_prices.clear();
        for store in &mut self.doc.stores {
            if store.crypto_account_id.is_some() {
                store.crypto_account_id = None;
                store.archived = true;
            }
        }
        // At level 3 `doc.exchanges.bybit` is only an index of the keys in
        // the device file, which goes with the wipe: an index of nothing
        // would show wallets as keyed when no key is left. Below level 3 the
        // document holds the user's own old keys, which are not the module's
        // and are left alone.
        if self.keys_on_device() {
            self.doc.exchanges.bybit.clear();
            // And the session forgets what the file held, so the save does
            // not write a trimmed file back over the one about to go.
            self.device_keys = super::super::exchange_keys::DeviceKeys::default();
        }
        // An archived wallet is no part of the emergency fund, and the
        // capital is read off `doc.holdings`, not off the flags.
        self.prune_emergency_fund();
        self.recompute_holdings();
        counts
    }

    /// The wallets the module leads today.
    fn crypto_engine_wallets(&self) -> Vec<CryptoWipedWallet> {
        self.doc
            .stores
            .iter()
            .filter(|store| store.crypto_account_id.is_some())
            .map(|store| CryptoWipedWallet {
                store_id: store.id.clone(),
                name: store.name.clone(),
            })
            .collect()
    }

    /// What the capital becomes minus what it is now, worked out on a copy of
    /// the vault in memory.
    ///
    /// It is not simply "minus what the module holds": the wallets kept by
    /// hand that the move archived yield their coins to the module
    /// (`super::transition`), and once the module is gone they count again.
    /// Only running the wipe on a copy answers that.
    fn crypto_wipe_value_delta(&self) -> Amount {
        let before = self.net_worth().net;
        let mut copy = self.sandbox();
        copy.wipe_crypto_document();
        copy.net_worth().net - before
    }

    fn crypto_counts(&self) -> CryptoWipeCounts {
        self.crypto_counts_of(&self.doc)
    }

    fn crypto_counts_of(&self, doc: &crate::model::vault::VaultDoc) -> CryptoWipeCounts {
        let count = |len: usize| u32::try_from(len).unwrap_or(u32::MAX);
        CryptoWipeCounts {
            accounts: count(doc.crypto_accounts.len()),
            events: count(doc.crypto_events.len()),
            overrides: count(doc.crypto_overrides.len()),
            tokens: count(doc.crypto_tokens.len()),
            positions: count(doc.crypto_positions.len()),
            snapshots: count(doc.crypto_snapshots.len()),
            cursors: count(doc.crypto_cursors.len()),
            prices: count(doc.crypto_prices.len()),
        }
    }

    /// Whether this device keeps a secrets file next to the vault.
    fn device_secrets_present(&self) -> bool {
        !self.sandbox && device_secrets::secrets_path(&self.path).exists()
    }

    /// RFC 3339 of the newest backup next to the vault, if the folder can be
    /// read at all.
    fn newest_backup_at(&self) -> Option<String> {
        self.list_backups()
            .ok()?
            .into_iter()
            .next()
            .map(|file| file.modified_at)
    }
}

impl CryptoWipeCounts {
    /// Whether the module stored nothing at all.
    fn is_empty(&self) -> bool {
        *self == CryptoWipeCounts::default()
    }
}

#[cfg(test)]
#[path = "wipe_tests.rs"]
mod tests;
