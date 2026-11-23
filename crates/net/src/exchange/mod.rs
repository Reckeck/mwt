//! Exchange integrations.
//!
//! Everything here is read-only by construction: the modules expose no
//! trading, funding or withdrawal endpoints, so a leaked key cannot move funds.

pub(crate) mod bybit;

pub use bybit::{BybitClient, Category, CoinBalance};
pub use mwt_model::{EarnYield, Execution, InterTransfer, TradePrefill};
