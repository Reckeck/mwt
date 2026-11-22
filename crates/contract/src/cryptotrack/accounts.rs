//! IPC types of the addresses and exchanges the crypto module follows.
//!
//! Owner: spec E9. A connected address is an ordinary wallet of kind `Crypto`
//! whose `Store::crypto_account_id` points at the account it shows, and an
//! exchange one of kind `Exchange` (decision P5 of 2026-11-11): there is no
//! separate crypto section, so these DTOs describe what the wallet page reads,
//! not a screen of its own.
//!
//! Commands: `crates/app/src/commands/cryptotrack/accounts.rs`.

use std::fmt;

use serde::{Deserialize, Serialize};

use mwt_base::Amount;
use mwt_model::Chain;

/// The message of the refusal when a pasted string looks like a seed phrase or
/// a private key. The text never carries the string itself, and the browser
/// mock and the form answer with the same words.
pub const SECRET_REFUSED: &str =
    "crypto: this looks like a seed phrase or a private key; it was not accepted and not stored";

/// A token of the account that waits for the user's decision, with how much
/// of it arrived. The symbol, the contract's warnings and the status come from
/// `crypto_list_tokens`: this is only what that list cannot say.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct QuarantinedHolding {
    pub chain: Chain,
    pub contract: String,
    /// What came in, minus what went out, in whole units. Never counted in
    /// the address's value while the token is in quarantine.
    pub quantity: Amount,
    /// When the first operation with this token happened (RFC3339 UTC).
    pub first_seen: String,
}

/// How an address's value moved over about a month, from the module's own daily snapshots.
///
/// Only ever made from two real snapshots of the account: the latest one, and the latest one at
/// or before 30 days ago (no more than 7 days older than that day). With no snapshot that old a
/// number would come from a short window, so there is no `MonthChange` at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct MonthChange {
    /// Value now minus value then, USD.
    pub change_usd: Amount,
    /// The same as a percent of the value then; `None` when that was zero.
    #[ts(optional)]
    pub change_pct: Option<Amount>,
    /// The day of the older snapshot (RFC3339 UTC).
    pub since: String,
}

/// One followed account as the wallet page reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoAccountView {
    pub id: String,
    pub name: String,
    pub chain: Chain,
    pub address: String,
    /// The source that reads it (`bybit` for an exchange); `None` is the
    /// network's public explorer.
    pub source: Option<String>,
    pub archived: bool,
    pub note: String,
    /// The date the module follows the address from (RFC3339 UTC).
    pub created_at: String,
    /// The wallet that shows this account (`Store::crypto_account_id`).
    pub store_id: Option<String>,
    /// Tokens of this account that wait for a decision.
    pub quarantine: Vec<QuarantinedHolding>,
    /// The month's change in its value; `None` when the snapshots do not reach back far enough.
    #[ts(optional)]
    pub month_change: Option<MonthChange>,
}

/// Connect a public address, read-only. There is no field for a seed phrase
/// or a private key anywhere in this contract, and a string that looks like
/// one is refused (`SECRET_REFUSED`) and never stored.
#[derive(Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ConnectAddressInput {
    pub chain: Chain,
    pub address: String,
    /// The wallet's name; empty takes the shortened address.
    pub name: String,
    /// `YYYY-MM-DD`: operations before it are not loaded and the balance of
    /// that day is written as one opening adjustment. `None` reads the whole
    /// history.
    #[ts(optional)]
    pub start_date: Option<String>,
}

// The address and the name are whatever the user pasted until they have been
// checked (a pasted secret may land in either), so neither reaches a log line
// through `{:?}`.
impl fmt::Debug for ConnectAddressInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectAddressInput")
            .field("chain", &self.chain)
            .field("address", &"<hidden>")
            .field("name", &"<hidden>")
            .field("start_date", &self.start_date)
            .finish()
    }
}

/// What connecting created: the account and the wallet that shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ConnectAddressResult {
    pub account_id: String,
    pub store_id: String,
}

/// Rename an account, change its note or stop counting it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateAccountInput {
    pub account_id: String,
    #[ts(optional)]
    pub name: Option<String>,
    #[ts(optional)]
    pub note: Option<String>,
    #[ts(optional)]
    pub archived: Option<bool>,
}

/// Which account a command is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoAccountRefInput {
    pub account_id: String,
}

#[cfg(test)]
mod debug_tests {
    use super::*;

    #[test]
    fn debug_of_connect_address_hides_the_address_and_the_name() {
        let input = ConnectAddressInput {
            chain: Chain::Ethereum,
            address: "0xaddress-secret".into(),
            name: "pasted-name-secret".into(),
            start_date: Some("2026-01-01".into()),
        };
        let text = format!("{input:?}");
        assert!(!text.contains("secret"), "{text}");
        assert!(text.contains("2026-01-01"));
    }
}
