//! Where the price of a position comes from, said next to the price itself:
//! a hand-set value, an FX rate, a provider's quote, the price of the last
//! transaction, or the dollar peg of a stablecoin.

use crate::Amount;
use crate::model::transaction::LegRole;
use crate::pricing::history::secs_or_now;

use crate::kernel::books::Books;
use crate::model::vault::VaultDoc;
use crate::money::rates::{latest_fx_point, per_usd};

/// How long ago the price a 24-hour change is measured against may be.
const DAY_SECS: i64 = 24 * 3600;

/// The kind of price a position is valued at.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS,
)]
#[serde(rename_all = "snake_case")]
pub enum PriceOrigin {
    /// Set by hand (a flat, a car).
    Manual,
    /// A currency at its FX rate.
    Fiat,
    /// A provider's quote from the price cache.
    Market,
    /// The price the last transaction with the asset recorded.
    Stamp,
    /// A dollar stablecoin nobody quoted yet: one dollar.
    Peg,
    /// No price at all.
    #[default]
    None,
}

/// Where a price came from and when, for the hint next to it.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PriceInfo {
    pub origin: PriceOrigin,
    /// The provider (`coingecko`, `coincap`, `binance`, an FX source); `None`
    /// when the origin says it all.
    pub source: Option<String>,
    /// RFC3339 moment the price is from; `None` for a hand-set price or the peg.
    pub at: Option<String>,
}

/// A position's current price with its story.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PriceQuote {
    pub usd: Amount,
    pub info: PriceInfo,
    /// Percent over the last 24 hours: the provider's figure, else from the
    /// cached history.
    pub change_24h_pct: Option<Amount>,
}

/// The current USD price of one unit, and where it came from; the hand-set
/// price is the one thing only the vault can value, so it is asked from `books`.
pub(crate) fn price_quote_for(books: &Books, asset_key: &str) -> PriceQuote {
    price_quote(
        books.doc(),
        asset_key,
        books.vault().manual_price_usd(asset_key),
    )
}

/// The current USD price of one unit, and where it came from. The order is
/// the one valuation has always used: hand-set (`manual_usd`, the vault's
/// answer, already in USD), FX, cached quote, last transaction, dollar peg.
fn price_quote(doc: &VaultDoc, asset_key: &str, manual_usd: Option<Amount>) -> PriceQuote {
    if let Some(usd) = manual_usd {
        return PriceQuote {
            usd,
            info: PriceInfo {
                origin: PriceOrigin::Manual,
                ..PriceInfo::default()
            },
            change_24h_pct: None,
        };
    }
    if let Some(code) = asset_key.strip_prefix("fiat:") {
        return fiat_quote(doc, code);
    }
    let latest = doc
        .price_cache
        .iter()
        .filter(|point| point.asset_key == asset_key)
        .max_by(|left, right| left.ts.cmp(&right.ts));
    if let Some(point) = latest {
        let change = point
            .change_24h_pct
            .or_else(|| change_from_history(doc, asset_key, point.value_usd, &point.ts));
        return PriceQuote {
            usd: point.value_usd,
            info: PriceInfo {
                origin: PriceOrigin::Market,
                // The old generic label names no provider.
                source: (point.source != "cryptoprice").then(|| point.source.clone()),
                at: Some(point.ts.clone()),
            },
            change_24h_pct: change,
        };
    }
    // A crypto token priced only by the history cache (`crypto_prices`):
    // its newest point, with the moment, so the hint shows how old it is.
    let historic = doc
        .crypto_prices
        .iter()
        .filter(|point| point.token_key == asset_key && !point.value_usd.is_zero())
        .max_by(|left, right| left.ts.cmp(&right.ts));
    if let Some(point) = historic {
        return PriceQuote {
            usd: point.value_usd,
            info: PriceInfo {
                origin: PriceOrigin::Market,
                source: Some(point.source.clone()),
                at: Some(point.ts.clone()),
            },
            change_24h_pct: None,
        };
    }
    if let Some((at, usd)) = last_stamp(doc, asset_key) {
        return PriceQuote {
            usd,
            info: PriceInfo {
                origin: PriceOrigin::Stamp,
                source: None,
                at: Some(at),
            },
            change_24h_pct: None,
        };
    }
    if crate::exchange::is_usd_stablecoin(asset_key) {
        return PriceQuote {
            usd: Amount::new(rust_decimal::Decimal::ONE),
            info: PriceInfo {
                origin: PriceOrigin::Peg,
                ..PriceInfo::default()
            },
            change_24h_pct: None,
        };
    }
    PriceQuote::default()
}

fn fiat_quote(doc: &VaultDoc, code: &str) -> PriceQuote {
    let code = code.to_lowercase();
    if code == "usd" {
        return PriceQuote {
            usd: Amount::new(rust_decimal::Decimal::ONE),
            info: PriceInfo {
                origin: PriceOrigin::Fiat,
                ..PriceInfo::default()
            },
            change_24h_pct: None,
        };
    }
    match (per_usd(doc, &code), latest_fx_point(doc, &code)) {
        (Some(rate), Some(point)) => PriceQuote {
            usd: Amount::new(rust_decimal::Decimal::ONE) / rate,
            info: PriceInfo {
                origin: PriceOrigin::Fiat,
                source: Some(point.source.clone()),
                at: Some(point.ts.clone()),
            },
            change_24h_pct: None,
        },
        _ => PriceQuote::default(),
    }
}

/// The change against the newest cached price at least a day older.
fn change_from_history(
    doc: &VaultDoc,
    asset_key: &str,
    now_usd: Amount,
    now_ts: &str,
) -> Option<Amount> {
    let day_before = secs_or_now(now_ts) - DAY_SECS;
    let then = doc
        .price_cache
        .iter()
        .filter(|point| point.asset_key == asset_key && secs_or_now(&point.ts) <= day_before)
        .max_by(|left, right| left.ts.cmp(&right.ts))?;
    if then.value_usd.is_zero() {
        return None;
    }
    // Prices come from the network: a ratio a decimal cannot hold is no
    // number, not a panic.
    let ratio = now_usd.checked_div(then.value_usd)?;
    let change = ratio.checked_sub(Amount::new(rust_decimal::Decimal::ONE))?;
    change
        .checked_mul(Amount::new(rust_decimal::Decimal::from(100)))
        .map(|pct| pct.round(2))
}

/// The last price a transaction recorded for the asset, with its moment.
pub(crate) fn last_stamp(doc: &VaultDoc, asset_key: &str) -> Option<(String, Amount)> {
    doc.transactions
        .iter()
        .filter_map(|tx| {
            tx.legs()
                .into_iter()
                .find(|leg| leg.asset.key() == asset_key && leg.role == LegRole::Principal)
                .and_then(|leg| leg.price_usd)
                .map(|price| (tx.meta().timestamp.clone(), price))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
}
