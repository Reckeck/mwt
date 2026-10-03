//! Strategy Lab: strategies & paper-account market simulation.
//!
//! The model (`Strategy`, `SimRun`, …) is defined here so vault files stay
//! forward-compatible; the engine lives in `engine.rs`.

pub mod engine;

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::store::AssetRef;
use crate::model::vault::now_iso;

/// A price feed for simulation: real historical candles (Binance/CoinGecko)
/// or synthetic (GBM) when offline.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    #[default]
    Real,
    Synthetic,
}

/// Trading signal definitions keyed by strategy.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct Rule {
    pub indicator: String,
    pub field: String,
    pub op: String,
    pub value: Amount,
}

/// A runnable trading strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct Strategy {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub chain: Option<Chain>,
    pub source: DataSource,
    pub initial_capital: Amount,
    pub asset: AssetRef,
    /// Rules evaluated on indicator streams to decide entries/exits.
    pub entry_rules: Vec<Rule>,
    pub exit_rules: Vec<Rule>,
    pub stop_loss_pct: Option<Amount>,
    pub take_profit_pct: Option<Amount>,
    pub position_size_pct: Amount,
    pub created_at: String,
}

impl Default for Strategy {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Unnamed strategy".into(),
            symbol: "BTC".into(),
            chain: None,
            source: DataSource::Synthetic,
            initial_capital: Amount::parse("10000").unwrap_or_default(),
            asset: AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin),
            entry_rules: Vec::new(),
            exit_rules: Vec::new(),
            stop_loss_pct: None,
            take_profit_pct: None,
            position_size_pct: Amount::parse("0.5").unwrap_or_default(),
            created_at: now_iso(),
        }
    }
}

/// One executed paper trade inside a run.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SimTrade {
    pub id: String,
    pub ts: String,
    pub side: String, // "buy" | "sell"
    pub price: Amount,
    pub qty: Amount,
    pub value: Amount,
    pub fee: Amount,
}

/// Result of running a strategy over a price stream.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SimRun {
    pub id: String,
    pub strategy_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub initial_capital: Amount,
    pub final_equity: Amount,
    pub total_return_pct: Amount,
    pub max_drawdown_pct: Amount,
    pub win_rate_pct: Amount,
    pub total_trades: u32,
    pub equity_curve: Vec<EquityPoint>,
    pub trades: Vec<SimTrade>,
    pub synthetic: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct EquityPoint {
    pub ts: String,
    pub equity: Amount,
    pub value: Amount,
}

impl EquityPoint {
    pub fn new(ts: String, equity: Amount) -> Self {
        Self {
            ts,
            equity,
            value: Amount::ZERO,
        }
    }
}
