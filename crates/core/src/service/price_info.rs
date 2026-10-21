//! Where the price of a position comes from, said next to the price itself:
//! a hand-set value, an FX rate, a provider's quote, the price of the last
//! transaction, or the dollar peg of a stablecoin.

use crate::Amount;
use crate::model::transaction::LegRole;
use crate::pricing::history::secs_or_now;

use super::UnlockedVault;

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

impl UnlockedVault {
    /// The current USD price of one unit, and where it came from. The order is
    /// the one valuation has always used: hand-set, FX, cached quote, last
    /// transaction, dollar peg.
    pub(crate) fn price_quote_for(&self, asset_key: &str) -> PriceQuote {
        if let Some(usd) = self.manual_price_usd(asset_key) {
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
            return self.fiat_quote(code);
        }
        let latest = self
            .doc
            .price_cache
            .iter()
            .filter(|point| point.asset_key == asset_key)
            .max_by(|left, right| left.ts.cmp(&right.ts));
        if let Some(point) = latest {
            let change = point
                .change_24h_pct
                .or_else(|| self.change_from_history(asset_key, point.value_usd, &point.ts));
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
        if let Some((at, usd)) = self.last_stamp(asset_key) {
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

    fn fiat_quote(&self, code: &str) -> PriceQuote {
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
        let latest = self
            .doc
            .fx_cache
            .iter()
            .filter(|point| point.currency == code && !point.per_usd.is_zero())
            .max_by(|left, right| left.ts.cmp(&right.ts));
        match latest {
            Some(point) => PriceQuote {
                usd: Amount::new(rust_decimal::Decimal::ONE) / point.per_usd,
                info: PriceInfo {
                    origin: PriceOrigin::Fiat,
                    source: Some(point.source.clone()),
                    at: Some(point.ts.clone()),
                },
                change_24h_pct: None,
            },
            None => PriceQuote::default(),
        }
    }

    /// The change against the newest cached price at least a day older.
    fn change_from_history(
        &self,
        asset_key: &str,
        now_usd: Amount,
        now_ts: &str,
    ) -> Option<Amount> {
        let day_before = secs_or_now(now_ts) - DAY_SECS;
        let then = self
            .doc
            .price_cache
            .iter()
            .filter(|point| point.asset_key == asset_key && secs_or_now(&point.ts) <= day_before)
            .max_by(|left, right| left.ts.cmp(&right.ts))?;
        if then.value_usd.is_zero() {
            return None;
        }
        Some(
            ((now_usd / then.value_usd - Amount::new(rust_decimal::Decimal::ONE))
                * Amount::new(rust_decimal::Decimal::from(100)))
            .round(2),
        )
    }

    /// The last price a transaction recorded for the asset, with its moment.
    pub(crate) fn last_stamp(&self, asset_key: &str) -> Option<(String, Amount)> {
        self.doc
            .transactions
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::vault::PricePoint;
    use crate::service::test_support::*;

    fn point(key: &str, usd: &str, ts: &str, source: &str, change: Option<&str>) -> PricePoint {
        PricePoint {
            asset_key: key.into(),
            value_usd: amount(usd),
            ts: ts.into(),
            source: source.into(),
            change_24h_pct: change.map(amount),
        }
    }

    #[test]
    fn a_cached_quote_reports_its_provider() {
        let (_dir, mut vault) = empty_vault();
        vault.apply_prices(&[point(
            "coin:bitcoin",
            "50000",
            "2026-10-19T10:00:00Z",
            "coingecko",
            Some("2.5"),
        )]);
        let quote = vault.price_quote_for("coin:bitcoin");
        assert_eq!(quote.info.origin, PriceOrigin::Market);
        assert_eq!(quote.info.source.as_deref(), Some("coingecko"));
        assert_eq!(quote.change_24h_pct, Some(amount("2.5")));
    }

    #[test]
    fn the_24h_change_falls_back_to_history() {
        let (_dir, mut vault) = empty_vault();
        vault.apply_prices(&[point(
            "coin:bitcoin",
            "40000",
            "2026-10-17T10:00:00Z",
            "cryptoprice",
            None,
        )]);
        vault.apply_prices(&[point(
            "coin:bitcoin",
            "50000",
            "2026-10-19T10:00:00Z",
            "cryptoprice",
            None,
        )]);
        let quote = vault.price_quote_for("coin:bitcoin");
        assert_eq!(quote.change_24h_pct, Some(amount("25")));
        // The old generic label names no provider.
        assert_eq!(quote.info.source, None);
    }

    #[test]
    fn an_unquoted_stablecoin_is_peg_valued() {
        let (_dir, vault) = empty_vault();
        let quote = vault.price_quote_for("coin:tether");
        assert_eq!(quote.info.origin, PriceOrigin::Peg);
        assert_eq!(quote.usd, amount("1"));
    }

    #[test]
    fn only_a_stamp_reports_stamp_origin() {
        let (_dir, mut vault) = empty_vault();
        let wallet = store(&mut vault, "Cold", StoreKind::Crypto, None);
        vault
            .add_transaction(income_of(&wallet, btc(), "1", "30000", "gift"))
            .unwrap();
        let quote = vault.price_quote_for(&btc().key());
        assert_eq!(quote.info.origin, PriceOrigin::Stamp);
        assert_eq!(quote.usd, amount("30000"));
    }

    #[test]
    fn a_manual_price_reports_manual_origin() {
        let (_dir, mut vault) = empty_vault();
        assert_eq!(
            vault.price_quote_for("asset:FLAT").info.origin,
            PriceOrigin::None
        );
        vault
            .doc
            .manual_prices
            .push(crate::model::vault::ManualPrice {
                asset_key: "asset:FLAT".into(),
                unit_price: amount("100000"),
                currency: "usd".into(),
                updated_at: "2026-10-01T00:00:00Z".into(),
            });
        let quote = vault.price_quote_for("asset:FLAT");
        assert_eq!(quote.info.origin, PriceOrigin::Manual);
        assert_eq!(quote.usd, amount("100000"));
    }
}
