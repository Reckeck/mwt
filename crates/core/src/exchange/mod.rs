//! Exchange integrations.
//!
//! Everything here is read-only by construction: the modules expose no
//! trading, funding or withdrawal endpoints, so a leaked key cannot move funds.

pub mod bybit;

pub use bybit::{
    BybitClient, Category, CoinBalance, EarnYield, Execution, InterTransfer, TradePrefill,
};
