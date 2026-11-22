//! IPC types of who an account dealt with: the user's other addresses and
//! exchanges, the protocols met, and the fiat operations of the same chain.
//!
//! Owner: spec E15. Relations are computed from the events every time and
//! never stored, and every row opens the operations with the filter that
//! produced it.
//!
//! Commands: `crates/app/src/commands/cryptotrack/relations.rs`.

use serde::{Deserialize, Serialize};

use crate::cryptotrack::links::CryptoPairFiat;
use crate::cryptotrack::operations::CryptoOpFilter;

/// Whose relations to read: one followed account (the wallet's own).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoRelationsInput {
    pub account_id: String,
}

/// One counterparty or protocol, with the filter that lists its operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoRelationRow {
    /// Unique within its group.
    pub key: String,
    /// What to show: an account's or a contact's name, a protocol.
    pub label: String,
    /// The address behind the row, for an address row.
    #[serde(default)]
    #[ts(optional)]
    pub address: Option<String>,
    /// How many visible operations the filter lists.
    pub count: u32,
    /// RFC3339 of the latest of them.
    pub last_time: String,
    /// The operations filter this row opens, the account already in it.
    pub filter: CryptoOpFilter,
}

/// A fiat record paired with one operation of the account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoFiatRelation {
    pub event_id: String,
    pub fiat: CryptoPairFiat,
    /// The filter to open the operation with: the account only; the card is
    /// opened by `event_id` (`?op=`).
    pub filter: CryptoOpFilter,
}

/// One group of the tab, shaped like the app's other relation groups
/// (`service::relations::RelationGroup`): the answer for an account with years
/// of history has to stay bounded, so the items are capped and `total` is how
/// many there are in all. The screen names the rest with the same
/// `entity.more` phrase — a list cut in silence tells the user something
/// untrue about their own data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoRelationGroup {
    pub items: Vec<CryptoRelationRow>,
    pub total: u32,
}

/// The fiat group, capped and counted the same way.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoFiatGroup {
    pub items: Vec<CryptoFiatRelation>,
    pub total: u32,
}

/// What an account dealt with. Never stored.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoRelations {
    /// The user's other followed addresses it moved coins with.
    pub own_addresses: CryptoRelationGroup,
    /// Counterparties the user named an exchanger.
    pub exchanges: CryptoRelationGroup,
    pub protocols: CryptoRelationGroup,
    pub fiat: CryptoFiatGroup,
}
