//! IPC types of wiping the module out of the vault: what a wipe would remove,
//! and the report of what it did.
//!
//! Owner: spec E12. A wipe clears every `crypto_*` field of the document,
//! marks `crypto_wiped_at` so another device cannot hand the data back, and
//! only then may the vault's protection go below level 3
//! (`doc_carries_crypto_data` of `mwt-vault`).
//!
//! Commands: `crates/app/src/commands/cryptotrack/wipe.rs`.

use serde::{Deserialize, Serialize};

use mwt_base::Amount;

/// How many records each collection of the module holds, so the confirmation
/// says what goes in numbers rather than in words.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CryptoWipeCounts {
    /// Followed addresses.
    pub accounts: u32,
    /// Operations the module read from its sources.
    pub events: u32,
    /// The user's corrections over what a source reported.
    pub overrides: u32,
    /// Tokens the module met, quarantine included.
    pub tokens: u32,
    /// Holdings inside protocols.
    pub positions: u32,
    /// Points of the module's own value history.
    pub snapshots: u32,
    /// How far each source has read: this device's progress, never synced.
    pub cursors: u32,
    /// Cached token quotes.
    pub prices: u32,
}

/// A wallet the module leads: a `Store` of kind `Crypto` carrying
/// `crypto_account_id`.
///
/// A wipe does not delete it. The wallet loses the link to the module and is
/// archived, because a wallet carries history and other records point at it,
/// so deleting one would take fiat operations and holdings with it.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CryptoWipedWallet {
    pub store_id: String,
    pub name: String,
}

/// What a wipe would remove, counted on demand and stored nowhere.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CryptoWipePreview {
    pub counts: CryptoWipeCounts,
    /// The wallets that lose their link to the module and are archived.
    pub wallets: Vec<CryptoWipedWallet>,
    /// The currency `value_delta` is in.
    pub base_currency: String,
    /// What the capital becomes minus what it is now: negative when the
    /// module holds something. Counted on a copy of the vault in memory, so
    /// it accounts for the hand-kept wallets that stop yielding their coins
    /// to the module and come back into the totals.
    pub value_delta: Amount,
    /// RFC 3339 of the newest backup next to the vault, if there is one. The
    /// wipe takes a backup of its own either way; this is what the user has
    /// already.
    pub backup_at: Option<String>,
    /// The device secrets file exists and goes with the wipe: the exchange
    /// keys, the per-network endpoints and the Blockscout key of this device.
    pub device_secrets: bool,
    /// Nothing of the module is here: a wipe would remove nothing.
    pub empty: bool,
}

/// The word the user types to confirm. The window asks for it in the user's
/// language; what reaches the vault is the one fixed token `WIPE_CONFIRM`
/// (`trade_core::service::cryptotrack::wipe`), so a stray call with no argument
/// cannot wipe anything.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CryptoWipeInput {
    pub confirm: String,
}

/// What the wipe did.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CryptoWipeReport {
    /// Records removed, per collection.
    pub counts: CryptoWipeCounts,
    /// The wallets that lost their link to the module and were archived.
    /// Nothing in them was deleted: their operations and history stay
    /// readable in the archive.
    pub wallets: Vec<CryptoWipedWallet>,
    pub base_currency: String,
    /// What the capital lost, in `base_currency`.
    pub value_delta: Amount,
    /// The backup the wipe took and opened before it removed anything, by
    /// file name next to the vault. `None` only when there was nothing to
    /// remove, so nothing was risked.
    pub backup: Option<String>,
    /// The device secrets file was removed: the exchange keys, the endpoints
    /// and the Blockscout key of this device went with it.
    pub device_secrets_removed: bool,
    /// RFC 3339 of the mark left behind, the one the other devices read.
    pub wiped_at: Option<String>,
    /// The protection may now go below level 3.
    pub can_lower_level: bool,
}
