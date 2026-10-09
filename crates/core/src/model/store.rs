use serde::{Deserialize, Serialize};

use crate::model::chain::Chain;
use schemars::JsonSchema;

/// What kind of container a store represents.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StoreKind {
    /// CEX (e.g. Binance, Bybit)
    Exchange,
    HotWallet,
    ColdWallet,
    Bank,
    #[default]
    Cash,
    /// Non-financial property valued by hand: a flat, a car, a collection.
    Asset,
    /// Money owed: a mortgage, a loan, a credit card. Its balance is negative.
    Liability,
}

impl StoreKind {
    /// Whether a new top-level store of this kind counts in the daily budget
    /// by default (plan D22): everyday money does, investments and debts do not.
    pub fn in_budget_by_default(self) -> bool {
        matches!(self, Self::Cash | Self::Bank)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LiabilityKind {
    Mortgage,
    Loan,
    CreditCard,
    Installment,
    Other,
}

/// Terms of a `liability` store. Informational, except `limit`, which caps
/// what an automation may charge to a credit card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct LiabilityTerms {
    pub kind: LiabilityKind,
    /// Annual interest rate in percent.
    pub rate: Option<crate::Amount>,
    /// Credit limit, in the store's currency.
    pub limit: Option<crate::Amount>,
    /// Day of the month the mandatory payment is due (1–31).
    pub due_day: Option<u8>,
    pub monthly_payment: Option<crate::Amount>,
}

/// Deepest allowed node depth; a root is depth 0, so the tree has three levels:
/// wallet → sub-account → sub-account.
pub const MAX_STORE_DEPTH: usize = 2;

/// A "store"/"platform" (хранилище) — a portfolio container.
/// Important: a store is NOT an asset — it holds holdings (`Holding`).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Store {
    pub id: String,
    pub name: String,
    pub kind: StoreKind,
    /// CSS-friendly accent color for the UI (hex, e.g. `#0ea5e9`).
    pub color: String,
    pub archived: bool,
    /// Free-form description shown in the UI.
    pub note: String,
    /// Parent store ID for sub-accounts; `None` for a top-level wallet. The tree
    /// rules (depth, unique sibling names, no cycles) are enforced on write by
    /// the service, see `MAX_STORE_DEPTH`.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Saved contact this wallet belongs to (an exchanger, a bank). Top-level only.
    pub contact_id: Option<String>,
    /// Loan terms; only on `liability` stores.
    pub liability: Option<LiabilityTerms>,
    /// Whether this store's income and spending count in the daily budget.
    pub in_budget: bool,
}

impl Store {
    pub fn new(name: &str, kind: StoreKind, color: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            kind,
            color: color.to_string(),
            archived: false,
            note: String::new(),
            parent_id: None,
            contact_id: None,
            liability: None,
            in_budget: kind.in_budget_by_default(),
        }
    }

    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }
}

/// Which stores a query covers. The one way every report filters by wallet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct StoreScope {
    /// Empty = every store.
    pub store_ids: Vec<String>,
    /// Expand each id to its whole subtree.
    pub include_children: bool,
}

impl Default for StoreScope {
    fn default() -> Self {
        Self {
            store_ids: Vec::new(),
            include_children: true,
        }
    }
}

impl StoreScope {
    pub fn store(id: &str, include_children: bool) -> Self {
        Self {
            store_ids: vec![id.to_string()],
            include_children,
        }
    }
}

/// A position of one asset inside one store (хранилище !== актив).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Holding {
    pub id: String,
    pub store_id: String,
    pub asset: AssetRef,
    pub quantity: crate::Amount,
    /// Optional manual average cost basis per unit (in base currency).
    pub avg_cost: Option<crate::Amount>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct AssetRef {
    pub symbol: String,
    pub name: String,
    /// e.g. `bitcoin` — used by CoinGecko.
    pub coingecko_id: Option<String>,
    pub chain: Option<Chain>,
    /// Token contract address (for tokens on EVM/Solana-like networks).
    pub contract: Option<String>,
    /// `base` quote currency symbol for FX peers (e.g. `usd`, `eur`).
    /// For crypto this is the network quote (e.g. `btc`, `eth`).
    /// Kept authoritative for display only.
    pub quote: String,
}

impl AssetRef {
    pub fn crypto(symbol: &str, name: &str, coingecko_id: Option<&str>, chain: Chain) -> Self {
        Self {
            symbol: symbol.to_uppercase(),
            name: name.to_string(),
            coingecko_id: coingecko_id.map(|id| id.to_string()),
            chain: Some(chain),
            contract: None,
            quote: symbol.to_uppercase(),
        }
    }

    pub fn fiat(symbol: &str, name: &str) -> Self {
        Self {
            symbol: symbol.to_uppercase(),
            name: name.to_string(),
            coingecko_id: None,
            chain: Some(Chain::Fiat),
            contract: None,
            quote: symbol.to_uppercase(),
        }
    }

    pub fn is_fiat(&self) -> bool {
        self.chain == Some(Chain::Fiat)
    }

    pub fn is_crypto(&self) -> bool {
        !self.is_fiat()
    }

    /// A perpetual futures contract. It is held like an asset (the open
    /// position), but its notional is not the user's money, so it never counts
    /// in portfolio value.
    pub fn perp(exchange: &str, symbol: &str) -> Self {
        Self {
            symbol: symbol.to_uppercase(),
            name: symbol.to_uppercase(),
            coingecko_id: None,
            chain: None,
            contract: Some(format!("{PERP_PREFIX}{}", exchange.to_lowercase())),
            quote: symbol.to_uppercase(),
        }
    }

    pub fn is_derivative(&self) -> bool {
        self.contract
            .as_deref()
            .is_some_and(|contract| contract.starts_with(PERP_PREFIX))
    }

    /// Stable key used for price cache lookups.
    pub fn key(&self) -> String {
        if self.is_derivative() {
            // perp:{exchange}:{symbol}
            return format!(
                "{}:{}",
                self.contract.as_deref().unwrap_or_default(),
                self.symbol
            );
        }
        match (&self.coingecko_id, &self.chain, &self.contract) {
            (Some(_), Some(chain), Some(contract)) => {
                format!("token:{}:{}:{}", chain_key(chain), self.symbol, contract)
            }
            (Some(id), _, _) => format!("coin:{id}"),
            (None, Some(chain), Some(contract)) => {
                format!("token:{}:{}:{}", chain_key(chain), self.symbol, contract)
            }
            _ => format!(
                "{}:{}",
                if self.is_fiat() { "fiat" } else { "asset" },
                self.symbol
            ),
        }
    }
}

const PERP_PREFIX: &str = "perp:";

fn chain_key(chain: &Chain) -> &'static str {
    match chain {
        Chain::Fiat => "fiat",
        _ => chain.label(),
    }
}
