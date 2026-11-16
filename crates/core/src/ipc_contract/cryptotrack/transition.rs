//! IPC types of entering the module: what leaves the totals, and the step
//! that archives the crypto wallets kept by hand once the vault is at
//! level 3.
//!
//! Owner: spec E11. The step is idempotent — a failure between the
//! re-encryption and the archiving is repaired at the next open — and it
//! deletes no record: a wallet is archived, its operations stay.
//!
//! A wallet the module reads itself is **not** in either list: an exchange of
//! a provider the module talks to stays, because it is or becomes a source of
//! the engine (its key can only be stored at level 3, so it arrives after the
//! move and says nothing at this point), and so does a wallet that already
//! shows a followed address. Nor is a wallet with nothing written in it: an
//! empty wallet is no record of the past. Only what the user wrote down by
//! hand is retired.
//!
//! Commands: `crates/app/src/commands/cryptotrack/transition.rs`.

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::store::StoreKind;

/// One wallet the move retires. It is archived and nothing in it is deleted:
/// its operations, holdings and history stay readable in the archive.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoRetiredWallet {
    pub store_id: String,
    pub name: String,
    pub kind: StoreKind,
    /// What this wallet holds today in the base currency: its share of
    /// [`CryptoTransitionPlan::leaving_base`]. Counted the way the capital
    /// counts it, so a futures contract (notional, not money) is not in it.
    pub leaving_base: Amount,
}

/// What entering the module would change, computed on demand and stored
/// nowhere: which hand-kept wallets the move archives and how much capital
/// leaves the totals with them. This is the number the wizard warns with
/// before the password is typed.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoTransitionPlan {
    /// The currency `leaving_base` and every `leaving_base` below are in.
    pub base_currency: String,
    /// The wallets about to be archived, in the order the wallet list has
    /// them. Empty when the user kept no crypto by hand.
    pub wallets: Vec<CryptoRetiredWallet>,
    /// The sum of the wallets' `leaving_base`: what the capital loses once
    /// the module accounts for anything.
    pub leaving_base: Amount,
}

/// What the archiving did. A second run reports `changed: false` and an empty
/// list: the step is idempotent, so the wizard, the raise and the next open
/// can all run it.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoArchiveReport {
    /// Whether this call archived anything at all.
    pub changed: bool,
    /// The wallets this call archived, with what each took out of the totals.
    pub wallets: Vec<CryptoRetiredWallet>,
}
