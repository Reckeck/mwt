#![warn(unreachable_pub)]
//! `mwt-net`: everything that talks to the network or parses what came back: the
//! price and FX sources (`PricingService`), the exchange clients, the data-source
//! extraction, market candles, the update manifest and the external-link checks.
//! It depends on `mwt-base` and `mwt-model` only. Modules are private; this root
//! re-lists every public item, and nothing else is public.

mod chain;
mod datasource;
mod exchange;
mod external_url;
mod market;
mod pricing;
#[cfg(test)]
mod props;
mod txinfo;
mod updates;

pub use chain::{HostLimiter, RpcCall, RpcClient, RpcConfig};
pub use datasource::{
    PreviewKind, SourcePreview, extract, parse_date_label, parse_number, preview,
};
pub use exchange::bybit::{KeyCheck, accounts_in_use};
pub use exchange::{
    BybitClient, Category, CoinBalance, EarnYield, Execution, InterTransfer, TradePrefill,
};
pub use external_url::{check_attachment_url, check_external_url};
pub use market::{Candle, fetch_candles, interval_str, iso_from_millis};
pub use mwt_model::PendingImportExt;
pub use pricing::history::{
    HISTORY_CAP, HOURLY_KEEP, RateIndex, merge_fx, merge_prices, secs_or_now, thin_fx, thin_prices,
    unix_secs,
};
pub use pricing::llama::llama_coin;
pub use pricing::{AssetSearchHit, PricingService, Quote};
pub use txinfo::{
    BtcPrevout, BtcStatus, BtcTx, BtcVin, BtcVout, SolMeta, SolTokenAmount, SolTokenBalance, SolTx,
    TxInfoClient,
};
pub use updates::{
    AppVersion, Channel, InstallKind, ManualHint, PlatformEntry, UpdateManifest, UpdateOffer,
    fetch_manifest, is_newer, manifest_url, offer_from_manifest,
};
