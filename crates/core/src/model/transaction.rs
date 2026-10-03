use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::store::AssetRef;

/// High-level meaning of a transaction.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum TxKind {
    /// Deposit, reward, airdrop — increases the store balance.
    #[default]
    Income,
    /// Withdrawal / spending — decreases the store balance.
    Expense,
    /// Displacement between two stores (two legs share `transfer_group`).
    Transfer,
    /// Trade between assets (modelled as out + in legs sharing a group).
    Trade,
    /// Standalone fee line.
    Fee,
    /// Synthetic trade made by the Strategy Lab / paper account.
    PaperTrade,
}

/// A single transaction record.
///
/// Sign convention:
/// - `amount > 0` — asset flows **into** `store_id` (unless kind is `Expense`, where positive means spent/out).
/// - `amount < 0` — asset flows **out** of `store_id`.
/// - `Transfer` legs: from-leg has negative amount on `store_id` + `to_store_id` set;
///   to-leg has positive amount with `store_id == source leg's to_store_id`.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct Transaction {
    pub id: String,
    pub kind: TxKind,
    pub asset: AssetRef,
    pub amount: Amount,
    /// Principal store (origin for expense, destination for income).
    pub store_id: String,
    /// Counterpart store for transfers/trades.
    pub to_store_id: Option<String>,
    pub network: Option<Chain>,
    /// On-chain tx hash (link to explorer).
    pub tx_hash: Option<String>,
    pub category: Option<String>,
    /// User note — REQUIRED field on the UI, free-form text here. (see README/docs)
    pub comment: String,
    /// ISO-8601 UTC timestamp, lexicographically sortable.
    pub timestamp: String,
    /// Fee paid, in `fee_asset` units (or the tx asset when None).
    pub fee: Option<Amount>,
    pub fee_asset: Option<String>,
    /// Group id shared across legs of the same transfer/trade/paper run.
    pub group_id: Option<String>,
    /// Price of one unit of `asset` in USD at tx time (stamped by backend).
    pub price_usd: Option<Amount>,
    /// Price of one unit of `asset` in the base currency at tx time.
    pub price_base: Option<Amount>,
    /// Optional user-provided exchange rate override at tx time (base per 1 usd).
    pub fx_override: Option<Amount>,
}

impl Transaction {
    pub fn new(
        kind: TxKind,
        asset: AssetRef,
        amount: Amount,
        store_id: &str,
        comment: &str,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            asset,
            amount,
            store_id: store_id.to_string(),
            to_store_id: None,
            network: None,
            tx_hash: None,
            category: None,
            comment: comment.trim().to_string(),
            timestamp: crate::model::vault::now_iso(),
            fee: None,
            fee_asset: None,
            group_id: None,
            price_usd: None,
            price_base: None,
            fx_override: None,
        }
    }

    pub fn explorer_url(&self) -> Option<String> {
        match (self.network, &self.tx_hash) {
            (Some(chain), Some(hash)) if !hash.is_empty() => chain.explorer_url(hash),
            _ => None,
        }
    }
}
