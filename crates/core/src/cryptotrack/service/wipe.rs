//! Rules of wiping the module out of the vault (spec E12): what the module holds,
//! which wallets it leads and what the wipe does to the document itself. The
//! scenarios (preview, backup, the write) are in `usecases::cryptotrack::wipe`.
//!
//! # The order, and why it is that order
//!
//! A wipe is the one operation of the module that cannot be taken back, so
//! the order is the whole design:
//!
//! 1. the word the user typed is checked — nothing else happens without it;
//! 2. there is a **backup**: the vault file as it is now, copied through the
//!    same `UnlockedVault::create_backup` the level-3 raise uses, then
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
//! The wipe leaves a `CryptoWipe` record in `crypto_wiped_at`, which syncs
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

use crate::model::vault::VaultDoc;

/// The token the vault accepts as "yes, wipe it".
///
/// The window asks the user to type a word in their own language and sends
/// this one once the word matches, so neither a mis-click nor a call with an
/// empty argument can wipe anything.
pub const WIPE_CONFIRM: &str = "WIPE";

/// Everything the wipe does to the document's own collections, and nothing
/// else: no file is touched, no mark is left and the holdings are not
/// recounted, so the preview can run it on a copy in memory.
///
/// `drop_exchange_index` is true at level 3, where `doc.exchanges.bybit` is
/// only an index of the keys in the device file, which goes with the wipe: an
/// index of nothing would show wallets as keyed when no key is left. Below
/// level 3 the document holds the user's own old keys, which are not the
/// module's and are left alone.
pub fn clear_module(doc: &mut VaultDoc, drop_exchange_index: bool) {
    doc.crypto_accounts.clear();
    doc.crypto_events.clear();
    doc.crypto_overrides.clear();
    doc.crypto_tokens.clear();
    doc.crypto_positions.clear();
    doc.crypto_snapshots.clear();
    doc.crypto_cursors.clear();
    doc.crypto_prices.clear();
    for store in &mut doc.stores {
        if store.crypto_account_id.is_some() {
            store.crypto_account_id = None;
            store.archived = true;
        }
    }
    if drop_exchange_index {
        doc.exchanges.bybit.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fixtures::full_doc;
    use crate::model::store::{Store, StoreKind};
    use crate::model::vault::ExchangeKeys;

    #[test]
    fn clearing_the_module_empties_it_and_archives_the_wallets_it_led() {
        let mut doc = full_doc();
        let mut led = Store::new("Cold", StoreKind::Crypto, "#000");
        led.crypto_account_id = Some("cacc-1".into());
        let led_id = led.id.clone();
        doc.stores.push(led);

        clear_module(&mut doc, false);

        assert!(doc.crypto_accounts.is_empty() && doc.crypto_events.is_empty());
        assert!(doc.crypto_overrides.is_empty() && doc.crypto_tokens.is_empty());
        assert!(doc.crypto_positions.is_empty() && doc.crypto_snapshots.is_empty());
        assert!(doc.crypto_cursors.is_empty() && doc.crypto_prices.is_empty());
        let store = doc.stores.iter().find(|store| store.id == led_id).unwrap();
        assert!(store.archived && store.crypto_account_id.is_none());
    }

    #[test]
    fn the_exchange_index_goes_only_when_it_is_the_device_files() {
        let mut doc = full_doc();
        doc.exchanges.bybit.push(ExchangeKeys {
            store_id: "wallet".into(),
            api_key: "abcd1234wxyz".into(),
            api_secret: "test-secret-no-real-key".into(),
            spot_store_id: None,
            futures_store_id: None,
        });
        let index = doc.exchanges.bybit.len();
        clear_module(&mut doc, false);
        assert_eq!(
            doc.exchanges.bybit.len(),
            index,
            "the user's own old keys stay"
        );
        clear_module(&mut doc, true);
        assert!(doc.exchanges.bybit.is_empty());
    }
}
