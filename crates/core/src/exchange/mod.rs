//! Exchange integrations.
//!
//! Everything here is read-only by construction: the modules expose no
//! trading, funding or withdrawal endpoints, so a leaked key cannot move funds.

pub mod bybit;
mod staging;

pub use bybit::{
    BybitClient, Category, CoinBalance, EarnYield, Execution, InterTransfer, TradePrefill,
};

pub use crate::model::coins::{is_usd_stablecoin, well_known_coin, well_known_coins};
