//! Price and FX sources: the facade of `mwt-net`. Every old path
//! `pricing::<module>::<item>` still resolves to the item of the crate.

pub use mwt_net::{AssetSearchHit, PricingService, Quote};

pub mod history {
    pub use mwt_net::{
        HISTORY_CAP, HOURLY_KEEP, RateIndex, merge_fx, merge_prices, secs_or_now, thin_fx,
        thin_prices, unix_secs,
    };
}

pub mod llama {
    pub use mwt_net::llama_coin;
}
