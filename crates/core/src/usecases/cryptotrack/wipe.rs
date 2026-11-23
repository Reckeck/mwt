//! Scenarios of wiping the module out of the vault (spec E12): the preview,
//! the backup the wipe stands on and the wipe itself. What the module holds
//! and what the wipe does to the document are in `cryptotrack::service::wipe`,
//! where the order of the steps is explained.
//!
//! The wipe is one write: the document is emptied through the rules, the mark
//! is left and `transact_now` puts the emptied document on the disk before the
//! device secrets file is removed.

use crate::Amount;
use crate::cryptotrack::service::wipe::{self as rules, WIPE_CONFIRM};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::wipe::{
    CryptoWipeCounts, CryptoWipePreview, CryptoWipeReport, CryptoWipedWallet,
};
use crate::ipc_contract::security::SecurityLevel;
use crate::model::cryptotrack::CryptoWipe;
use crate::model::vault::{VaultDoc, now_iso};
use crate::service::UnlockedVault;
use crate::service::security_level::level_rank;
use crate::usecases::money::capital::net_worth;
use crate::usecases::planning::emergency::prune_emergency_fund;
use crate::vault_io::{backups, device_secrets};

/// What a wipe would remove: the records per collection, the wallets that
/// would be unlinked and archived, what the capital would lose and which
/// backup the user has already.
pub fn crypto_wipe_preview(vault: &UnlockedVault) -> CryptoWipePreview {
    let counts = crypto_counts_of(&vault.doc);
    let wallets = engine_wallets(&vault.doc);
    let books = vault.books();
    let (base_currency, _) = books.base_and_fx();
    let base_currency = base_currency.to_string();
    let device_secrets = device_secrets_present(vault);
    let empty = counts_are_empty(&counts) && wallets.is_empty() && !device_secrets;
    CryptoWipePreview {
        counts,
        wallets,
        base_currency,
        value_delta: crypto_wipe_value_delta(vault),
        backup_at: newest_backup_at(vault),
        device_secrets,
        empty,
    }
}

/// Wipe the module. See `cryptotrack::service::wipe` for the order; `confirm`
/// must be [`WIPE_CONFIRM`].
///
/// Nothing is removed while the backup cannot be taken and read back, and
/// a vault that holds nothing of the module takes no backup at all: there
/// is nothing to risk and nothing to mark.
pub fn crypto_wipe_module(vault: &mut UnlockedVault, confirm: &str) -> Result<CryptoWipeReport> {
    wipe_inner(vault, confirm, backup_before_wipe)
}

/// [`crypto_wipe_module`] with the backup step handed in, so a test can watch
/// what a failed backup leaves behind: the answer must be "everything,
/// untouched".
pub(crate) fn wipe_inner(
    vault: &mut UnlockedVault,
    confirm: &str,
    backup: impl FnOnce(&mut UnlockedVault) -> Result<String>,
) -> Result<CryptoWipeReport> {
    if confirm != WIPE_CONFIRM {
        return Err(Error::InvalidInput(
            "the wipe of the crypto module was not confirmed".into(),
        ));
    }
    if vault.sandbox {
        return Err(Error::Unsupported);
    }
    let preview = crypto_wipe_preview(vault);
    if preview.empty {
        return Ok(CryptoWipeReport {
            base_currency: preview.base_currency,
            can_lower_level: can_lower_level(vault),
            ..CryptoWipeReport::default()
        });
    }

    let backup = backup(vault)?;

    let counts = wipe_crypto_document(vault);
    let wiped_at = now_iso();
    let mark = CryptoWipe {
        id: uuid::Uuid::new_v4().to_string(),
        at: wiped_at.clone(),
        device_id: vault.sync_device_id().to_string(),
    };
    vault.doc.crypto_wiped_at.push(mark);
    vault.transact_now(|tx| {
        tx.touch();
        Ok(())
    })?;

    // Only now, with the emptied document on the disk: the keys, the
    // endpoints and the Blockscout key of this device go as one file.
    device_secrets::remove(&vault.path)?;

    Ok(CryptoWipeReport {
        counts,
        wallets: preview.wallets,
        base_currency: preview.base_currency,
        value_delta: preview.value_delta,
        backup: Some(backup),
        device_secrets_removed: preview.device_secrets,
        wiped_at: Some(wiped_at),
        can_lower_level: can_lower_level(vault),
    })
}

/// Whether the vault may go below level 3 once the module is gone.
fn can_lower_level(vault: &UnlockedVault) -> bool {
    level_rank(vault.min_allowed_level()) < level_rank(SecurityLevel::Crypto)
}

/// The backup the wipe stands on: the file as it is now, copied and then
/// opened again with this session's password and compared with what is in
/// memory. Returns its file name.
///
/// A copy that cannot be opened, or that does not hold the module's data,
/// is no backup at all: the wipe stops and the document is still whole.
pub(crate) fn backup_before_wipe(vault: &mut UnlockedVault) -> Result<String> {
    let backup = vault.create_backup()?;
    let path = backups::resolve(&vault.path, &backup.name)?;
    let saved = vault.read_back_backup(&path)?;
    if crypto_counts_of(&saved) != crypto_counts_of(&vault.doc) {
        return Err(Error::Crypto(
            "the backup does not hold the crypto module: nothing was wiped".into(),
        ));
    }
    Ok(backup.name)
}

/// Everything the wipe does to the document itself, and nothing else: no
/// file is touched and no mark is left, so the preview can run it on a
/// copy in memory to see what the capital becomes. The caller writes.
pub(crate) fn wipe_crypto_document(vault: &mut UnlockedVault) -> CryptoWipeCounts {
    let on_device = vault.keys_on_device();
    let counts = crypto_counts_of(&vault.doc);
    rules::clear_module(&mut vault.doc, on_device);
    if on_device {
        // The session forgets what the file held, so the save does not write
        // a trimmed file back over the one about to go.
        vault.forget_device_keys();
    }
    // An archived wallet is no part of the emergency fund, and the capital is
    // read off `doc.holdings`, not off the flags.
    prune_emergency_fund(vault);
    crate::usecases::money::portfolio::recompute_holdings(vault);
    counts
}

/// What the capital becomes minus what it is now, worked out on a copy of
/// the vault in memory.
///
/// It is not simply "minus what the module holds": the wallets kept by
/// hand that the move archived yield their coins to the module
/// (`cryptotrack::service::transition`), and once the module is gone they
/// count again. Only running the wipe on a copy answers that.
fn crypto_wipe_value_delta(vault: &UnlockedVault) -> Amount {
    let before = net_worth(&vault.books()).net;
    let mut copy = vault.sandbox();
    wipe_crypto_document(&mut copy);
    net_worth(&copy.books()).net - before
}

/// Whether this device keeps a secrets file next to the vault.
fn device_secrets_present(vault: &UnlockedVault) -> bool {
    !vault.sandbox && device_secrets::secrets_path(&vault.path).exists()
}

/// RFC 3339 of the newest backup next to the vault, if the folder can be
/// read at all.
fn newest_backup_at(vault: &UnlockedVault) -> Option<String> {
    vault
        .list_backups()
        .ok()?
        .into_iter()
        .next()
        .map(|file| file.modified_at)
}

/// What the module holds in `doc`, per collection.
fn crypto_counts_of(doc: &VaultDoc) -> CryptoWipeCounts {
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

/// Whether the module stored nothing at all.
fn counts_are_empty(counts: &CryptoWipeCounts) -> bool {
    *counts == CryptoWipeCounts::default()
}

/// The wallets the module leads today.
fn engine_wallets(doc: &VaultDoc) -> Vec<CryptoWipedWallet> {
    doc.stores
        .iter()
        .filter(|store| store.crypto_account_id.is_some())
        .map(|store| CryptoWipedWallet {
            store_id: store.id.clone(),
            name: store.name.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fixtures::full_doc;
    use crate::model::store::{Store, StoreKind};

    #[test]
    fn the_counts_name_every_collection_and_an_empty_document_has_none() {
        assert!(counts_are_empty(&crypto_counts_of(&VaultDoc::default())));
        let counts = crypto_counts_of(&full_doc());
        assert!(!counts_are_empty(&counts));
        assert!(counts.accounts > 0 && counts.events > 0 && counts.tokens > 0);
    }

    #[test]
    fn the_wallets_the_module_leads_are_the_ones_with_a_followed_account() {
        let mut doc = VaultDoc::default();
        let mut led = Store::new("Cold", StoreKind::Crypto, "#000");
        led.crypto_account_id = Some("cacc-1".into());
        doc.stores = vec![led, Store::new("Bank", StoreKind::Bank, "#000")];
        let wallets = engine_wallets(&doc);
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].name, "Cold");
    }
}
