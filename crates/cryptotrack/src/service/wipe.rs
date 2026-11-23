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
//!    same `Session::create_backup` the level-3 raise uses, then
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
//! [`mwt_model::Store::crypto_account_id`] (decision P5 of
//! `U2-decisions.md`). The wipe **strips that link and archives the wallet**;
//! it deletes no `Store`. A wallet carries history — operations, holdings,
//! favourites, the capital structure all point at its id — so deleting one
//! would take records with it that the module never owned. Archiving is the
//! same answer the move *into* the module gives for the wallets kept by hand
//! (`super::transition`), and the report names every wallet it archived.
//!
//! Afterwards
//! `crate::service::security_level::doc_carries_crypto_data` is false and
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

use mwt_model::VaultDoc;

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
