use serde::{Deserialize, Serialize};

use crate::model::chain::Chain;

/// What kind of container a store represents.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum StoreKind {
    /// CEX (e.g. Binance, Bybit)
    Exchange,
    HotWallet,
    ColdWallet,
    Bank,
    #[default]
    Cash,
    /// Virtual money used by the Strategy Lab simulator.
    Paper,
}

/// A "store"/"platform" (хранилище) — a portfolio container.
/// Important: a store is NOT an asset — it holds holdings (`Holding`).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct Store {
    pub id: String,
    pub name: String,
    pub kind: StoreKind,
    /// CSS-friendly accent color for the UI (hex, e.g. `#0ea5e9`).
    pub color: String,
    pub archived: bool,
    /// Free-form description shown in the UI.
    pub note: String,
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
        }
    }
}

/// A position of one asset inside one store (хранилище !== актив).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct Holding {
    pub id: String,
    pub store_id: String,
    pub asset: AssetRef,
    pub quantity: crate::Amount,
    /// Optional manual average cost basis per unit (in base currency).
    pub avg_cost: Option<crate::Amount>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
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
            coingecko_id: coingecko_id.map(|s| s.to_string()),
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

    /// Stable key used for price cache lookups.
    pub fn key(&self) -> String {
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

fn chain_key(c: &Chain) -> &'static str {
    match c {
        Chain::Fiat => "fiat",
        _ => c.label(),
    }
}
