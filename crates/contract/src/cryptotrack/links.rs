//! IPC types of pointing a fiat record at a crypto operation, and back.
//!
//! Owner: spec E13. The pointer is `CryptoOverride::fiat_tx_id`: a visible
//! reference and nothing more. It joins no chain, changes no cost basis and no
//! sum; the chain of money (`flow_id`) is written by
//! `crypto_set_override` (spec E10) and by nothing here. The pointer lives on
//! the crypto side, in `crypto_overrides`, so it is gated below level 3 and
//! wiped with the module, and a fiat record never carries a chain hash.
//!
//! Commands: `crates/app/src/commands/cryptotrack/links.rs`.

use serde::{Deserialize, Serialize};

use crate::cryptotrack::operations::CryptoOpRow;
use mwt_base::Amount;

/// What the fiat record being edited looks like, so the picker can offer the
/// crypto operations that look like the same movement of money. The record may
/// not be saved yet, so this is a draft and not an id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoLinkCandidatesInput {
    /// RFC3339 of the fiat record.
    pub time: String,
    /// The fiat sum, when the form has one (the sign does not matter).
    #[serde(default)]
    #[ts(optional)]
    pub amount: Option<Amount>,
    /// The currency of `amount`.
    #[serde(default)]
    #[ts(optional)]
    pub currency: Option<String>,
}

/// One suggestion. Never applied by the application itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoLinkOption {
    pub row: CryptoOpRow,
    /// How many days apart the two are.
    pub days: u32,
    /// The fiat sum fits the coin's value that day within a few percent.
    pub fits: bool,
}

/// Point a crypto operation at a fiat record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoLinkInput {
    pub event_id: String,
    pub fiat_tx_id: String,
}

/// Remove the pointer of a crypto operation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoUnlinkInput {
    pub event_id: String,
}

/// Which pointers to read: the ones of one operation, or of one fiat record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoPairsInput {
    #[serde(default)]
    #[ts(optional)]
    pub event_id: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub fiat_tx_id: Option<String>,
}

/// The fiat side of a pointer, as much as a row needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoPairFiat {
    pub id: String,
    /// The wallet it belongs to.
    pub place: String,
    /// Signed: the way it moved for its own wallet.
    pub amount: Amount,
    pub currency: String,
    pub time: String,
    pub comment: String,
}

/// One pointer with both ends resolved. An end that no longer exists is
/// `None`: a calm «not found» for the screen, never an error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoPair {
    pub event_id: String,
    pub fiat_tx_id: String,
    #[serde(default)]
    #[ts(optional)]
    pub event: Option<CryptoOpRow>,
    #[serde(default)]
    #[ts(optional)]
    pub fiat: Option<CryptoPairFiat>,
}
