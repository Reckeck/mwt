//! Parsed, sorted rate series for repeated lookups — a replay asks thousands
//! of times, and parsing the cache on each ask would dominate it.

use std::collections::HashMap;

use mwt_base::Amount;
use mwt_base::time::unix_secs;
use mwt_model::CryptoPricePoint;
use mwt_model::{FxPoint, PricePoint};

/// The newest-point lookups over the FX, price and crypto-price caches.
#[derive(Debug, Default, Clone)]
pub struct RateIndex {
    series: HashMap<String, Vec<(i64, Amount)>>,
}

impl RateIndex {
    pub fn of_fx(cache: &[FxPoint]) -> Self {
        Self::of(
            cache
                .iter()
                .map(|point| (&point.currency, &point.ts, point.per_usd)),
        )
    }

    pub fn of_prices(cache: &[PricePoint]) -> Self {
        Self::of(
            cache
                .iter()
                .map(|point| (&point.asset_key, &point.ts, point.value_usd)),
        )
    }

    /// The historic crypto prices of `crypto_prices`, never thinned.
    pub fn of_crypto(cache: &[CryptoPricePoint]) -> Self {
        Self::of(
            cache
                .iter()
                .map(|point| (&point.token_key, &point.ts, point.value_usd)),
        )
    }

    fn of<'point>(points: impl Iterator<Item = (&'point String, &'point String, Amount)>) -> Self {
        let mut index = Self::default();
        for (series, ts, value) in points {
            if let Some(secs) = unix_secs(ts) {
                index.insert(series, secs, value);
            }
        }
        index.sort();
        index
    }

    /// Add a point; call [`RateIndex::sort`] before asking.
    pub fn insert(&mut self, key: &str, secs: i64, value: Amount) {
        if value.is_zero() {
            return;
        }
        self.series
            .entry(key.to_string())
            .or_default()
            .push((secs, value));
    }

    pub fn sort(&mut self) {
        for points in self.series.values_mut() {
            points.sort_by_key(|(secs, _)| *secs);
        }
    }

    /// The newest point at or before `secs`.
    pub fn at_or_before(&self, key: &str, secs: i64) -> Option<Amount> {
        self.point_before(key, secs).map(|(_, value)| value)
    }

    /// The nearest point at or before `secs`, else the first one after it.
    pub fn at(&self, key: &str, secs: i64) -> Option<Amount> {
        self.point_at(key, secs).map(|(_, value)| value)
    }

    /// Like [`RateIndex::at`], with the moment the point was taken.
    pub fn point_at(&self, key: &str, secs: i64) -> Option<(i64, Amount)> {
        self.point_before(key, secs)
            .or_else(|| self.series.get(key)?.first().copied())
    }

    fn point_before(&self, key: &str, secs: i64) -> Option<(i64, Amount)> {
        let points = self.series.get(key)?;
        let after = points.partition_point(|(point, _)| *point <= secs);
        after.checked_sub(1).map(|idx| points[idx])
    }

    pub fn latest(&self, key: &str) -> Option<Amount> {
        self.series.get(key)?.last().map(|(_, value)| *value)
    }
}
