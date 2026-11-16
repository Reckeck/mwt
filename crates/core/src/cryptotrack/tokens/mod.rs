//! Tokens the crypto module has met, and whether they count (spec E2).
//!
//! A token is identified by its network and contract
//! ([`TokenId`]), never by its symbol: a scam token is free to call itself
//! `USDC`. Its [`TokenStatus`] is derived, not stored:
//!
//! * the user's verdict, if any, wins: a field of the token's own record
//!   ([`TokenVerdict`]) that confirms or rejects it;
//! * otherwise a contract on the built-in list ([`known`]) is `Known`;
//! * anything else is `Quarantined`: kept in the list, out of every total.
//!
//! Registering a token again (a refetch, a re-read of the events) refreshes
//! its text and never touches `verdict` or `basis_usd`.

pub mod known;
pub mod text;

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::cryptotrack::{TokenId, TokenVerdict};
use crate::model::vault::VaultDoc;

/// Where a token stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum TokenStatus {
    /// On the built-in list.
    Known,
    /// Unknown contract: shown for review, counted nowhere.
    Quarantined,
    /// The user took it into the totals.
    Confirmed,
    /// The user hid it.
    Rejected,
}

impl TokenStatus {
    /// Only these two reach a total, a chart or a price request.
    pub fn counts_in_totals(self) -> bool {
        matches!(self, Self::Known | Self::Confirmed)
    }
}

/// The status of a token in a document.
pub fn status(doc: &VaultDoc, token: &TokenId) -> TokenStatus {
    let record = doc
        .crypto_tokens
        .iter()
        .find(|record| record.id() == *token);
    match record.and_then(|record| record.verdict) {
        Some(TokenVerdict::Confirmed) => TokenStatus::Confirmed,
        Some(TokenVerdict::Rejected) => TokenStatus::Rejected,
        None if known::is_known(token) => TokenStatus::Known,
        None => TokenStatus::Quarantined,
    }
}

/// The cost basis the user set for a token, USD per unit.
pub fn basis(doc: &VaultDoc, token: &TokenId) -> Option<Amount> {
    doc.crypto_tokens
        .iter()
        .find(|record| record.id() == *token)
        .and_then(|record| record.basis_usd)
}

/// The question every total asks about a holding.
pub fn counts_in_totals(doc: &VaultDoc, token: &TokenId) -> bool {
    status(doc, token).counts_in_totals()
}
