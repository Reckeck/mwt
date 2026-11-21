//! Exchange integrations: the facade of `mwt-net`. Every old path
//! `exchange::<module>::<item>` still resolves to the item of the crate.

pub use mwt_model::{is_usd_stablecoin, well_known_coin, well_known_coins};
pub use mwt_net::{
    BybitClient, Category, CoinBalance, EarnYield, Execution, InterTransfer, TradePrefill,
};

pub mod bybit {
    pub use mwt_model::{ACCOUNT_EARN, ACCOUNT_FUND, ACCOUNT_UNIFIED};
    pub use mwt_net::{CoinBalance, KeyCheck, accounts_in_use};
}

pub mod staging {
    pub use mwt_net::PendingImportExt;
}
