//! Rate history: how the FX and price caches keep past points and answer
//! "what was the rate then" (plan §6.1).
//!
//! The caches used to keep one point per currency or asset, so every refresh
//! overwrote the rate a past transaction needs. Now a refresh adds a point:
//! one per series per hour, the newest week at that resolution, older points
//! thinned to one a day, and never more than [`HISTORY_CAP`] per series.

use std::collections::{BTreeSet, HashMap};

use crate::Amount;
use crate::model::vault::{FxPoint, PricePoint};

/// Most points one currency or asset keeps (≈ 83 days hourly, years daily).
pub const HISTORY_CAP: usize = 2000;
/// The newest points kept at hourly resolution; older ones thin to one a day.
pub const HOURLY_KEEP: usize = 24 * 7;

const HOUR: i64 = 3600;
const DAY: i64 = 86_400;

/// Unix seconds of an RFC3339 timestamp. Plain string order is not enough:
/// `…:00Z` and `…:00.5Z` sort the wrong way round.
pub fn unix_secs(iso: &str) -> Option<i64> {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::parse(iso.trim(), &Rfc3339)
        .ok()
        .map(|moment| moment.unix_timestamp())
}

/// Unix seconds of `iso`, or now when it is empty or unreadable.
pub fn secs_or_now(iso: &str) -> i64 {
    unix_secs(iso).unwrap_or_else(|| time::OffsetDateTime::now_utc().unix_timestamp())
}

/// A cached point of one series (a currency or an asset key).
trait Point: Clone {
    fn series(&self) -> &str;
    fn ts(&self) -> &str;
    fn value(&self) -> Amount;
}

impl Point for FxPoint {
    fn series(&self) -> &str {
        &self.currency
    }
    fn ts(&self) -> &str {
        &self.ts
    }
    fn value(&self) -> Amount {
        self.per_usd
    }
}

impl Point for PricePoint {
    fn series(&self) -> &str {
        &self.asset_key
    }
    fn ts(&self) -> &str {
        &self.ts
    }
    fn value(&self) -> Amount {
        self.value_usd
    }
}

/// Add fresh FX points. Every currency is cached, but only the ones in
/// `keep_history` keep their past: a hundred and seventy currencies hourly
/// would bloat a vault that is re-encrypted on every write.
pub fn merge_fx(cache: &mut Vec<FxPoint>, fresh: &[FxPoint], keep_history: impl Fn(&str) -> bool) {
    merge(cache, fresh, keep_history);
}

/// Add fresh price points; every priced asset keeps its history.
pub fn merge_prices(cache: &mut Vec<PricePoint>, fresh: &[PricePoint]) {
    merge(cache, fresh, |_| true);
}

fn merge<P: Point>(cache: &mut Vec<P>, fresh: &[P], keep_history: impl Fn(&str) -> bool) {
    let touched: BTreeSet<String> = fresh
        .iter()
        .map(|point| point.series().to_string())
        .collect();
    if touched.is_empty() {
        return;
    }
    let (mut series_points, rest): (Vec<P>, Vec<P>) = std::mem::take(cache)
        .into_iter()
        .partition(|point| touched.contains(point.series()));
    *cache = rest;
    series_points.extend(fresh.iter().cloned());

    let mut by_series: HashMap<String, Vec<(i64, P)>> = HashMap::new();
    for point in series_points {
        if let Some(secs) = unix_secs(point.ts()) {
            by_series
                .entry(point.series().to_string())
                .or_default()
                .push((secs, point));
        }
    }
    for key in touched {
        let Some(mut points) = by_series.remove(&key) else {
            continue;
        };
        // Stable sort keeps a fresh point after a cached one of the same
        // second, so the fresh one wins the hourly bucket.
        points.sort_by_key(|(secs, _)| *secs);
        let mut points = last_per_bucket(points, HOUR);
        if !keep_history(&key) {
            points = points.split_off(points.len().saturating_sub(1));
        } else if points.len() > HOURLY_KEEP {
            let recent = points.split_off(points.len() - HOURLY_KEEP);
            points = last_per_bucket(points, DAY);
            points.extend(recent);
        }
        if points.len() > HISTORY_CAP {
            points.drain(..points.len() - HISTORY_CAP);
        }
        cache.extend(points.into_iter().map(|(_, point)| point));
    }
}

/// Keep the newest point of every `width`-second bucket. `points` is sorted.
fn last_per_bucket<P>(points: Vec<(i64, P)>, width: i64) -> Vec<(i64, P)> {
    let mut out: Vec<(i64, P)> = Vec::with_capacity(points.len());
    for (secs, point) in points {
        if out
            .last()
            .is_some_and(|(last, _)| last.div_euclid(width) == secs.div_euclid(width))
        {
            out.pop();
        }
        out.push((secs, point));
    }
    out
}

/// Parsed, sorted series for repeated lookups — a replay asks thousands of
/// times, and parsing the cache on each ask would dominate it.
#[derive(Debug, Default, Clone)]
pub struct RateIndex {
    series: HashMap<String, Vec<(i64, Amount)>>,
}

impl RateIndex {
    pub fn of_fx(cache: &[FxPoint]) -> Self {
        Self::of(cache)
    }

    pub fn of_prices(cache: &[PricePoint]) -> Self {
        Self::of(cache)
    }

    fn of<P: Point>(cache: &[P]) -> Self {
        let mut index = Self::default();
        for point in cache {
            if let Some(secs) = unix_secs(point.ts()) {
                index.insert(point.series(), secs, point.value());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(currency: &str, per_usd: &str, ts: &str) -> FxPoint {
        FxPoint {
            currency: currency.into(),
            per_usd: Amount::parse(per_usd).unwrap(),
            ts: ts.into(),
            source: "test".into(),
        }
    }

    fn iso(secs: i64) -> String {
        use time::format_description::well_known::Rfc3339;
        time::OffsetDateTime::from_unix_timestamp(secs)
            .unwrap()
            .format(&Rfc3339)
            .unwrap()
    }

    #[test]
    fn refresh_adds_history_instead_of_overwriting() {
        let mut cache = vec![fx("rub", "90", "2026-01-01T10:00:00Z")];
        merge_fx(
            &mut cache,
            &[fx("rub", "95", "2026-02-01T10:00:00Z")],
            |_| true,
        );
        assert_eq!(cache.len(), 2);
        let index = RateIndex::of_fx(&cache);
        let jan = unix_secs("2026-01-15T00:00:00Z").unwrap();
        assert_eq!(index.at("rub", jan), Some(Amount::parse("90").unwrap()));
        assert_eq!(index.latest("rub"), Some(Amount::parse("95").unwrap()));
    }

    #[test]
    fn one_point_per_hour_and_the_fresh_one_wins() {
        let mut cache = vec![fx("rub", "90", "2026-01-01T10:05:00Z")];
        merge_fx(
            &mut cache,
            &[fx("rub", "91", "2026-01-01T10:55:00.5Z")],
            |_| true,
        );
        assert_eq!(cache.len(), 1);
        assert_eq!(cache[0].per_usd, Amount::parse("91").unwrap());
    }

    #[test]
    fn untracked_currencies_keep_only_the_latest_rate() {
        let mut cache = vec![fx("thb", "36", "2026-01-01T10:00:00Z")];
        merge_fx(
            &mut cache,
            &[fx("thb", "35", "2026-02-01T10:00:00Z")],
            |code| code == "rub",
        );
        assert_eq!(cache.len(), 1);
        assert_eq!(cache[0].per_usd, Amount::parse("35").unwrap());
    }

    #[test]
    fn old_points_thin_to_daily_and_the_series_is_capped() {
        let start = unix_secs("2020-01-01T00:00:00Z").unwrap();
        // Hourly points for 200 days: far more than the cap.
        let fresh: Vec<FxPoint> = (0..24 * 200)
            .map(|hour| fx("rub", "90", &iso(start + hour * HOUR)))
            .collect();
        let mut cache = Vec::new();
        merge_fx(&mut cache, &fresh, |_| true);
        // A week hourly, the rest one a day.
        assert_eq!(cache.len(), HOURLY_KEEP + 200 - 7);
        assert!(cache.len() <= HISTORY_CAP);

        let years: Vec<FxPoint> = (0..24 * 3000)
            .step_by(24)
            .map(|hour| fx("eur", "0.9", &iso(start + hour * HOUR)))
            .collect();
        merge_fx(&mut cache, &years, |_| true);
        let eur = cache.iter().filter(|point| point.currency == "eur").count();
        assert_eq!(eur, HISTORY_CAP);
    }

    #[test]
    fn lookup_falls_back_to_the_first_later_point() {
        let cache = vec![
            fx("rub", "90", "2026-03-01T00:00:00Z"),
            fx("rub", "100", "2026-04-01T00:00:00Z"),
        ];
        let index = RateIndex::of_fx(&cache);
        let before_all = unix_secs("2025-01-01T00:00:00Z").unwrap();
        let between = unix_secs("2026-03-20T00:00:00Z").unwrap();
        assert_eq!(
            index.at("rub", before_all),
            Some(Amount::parse("90").unwrap())
        );
        assert_eq!(index.at_or_before("rub", before_all), None);
        assert_eq!(index.at("rub", between), Some(Amount::parse("90").unwrap()));
        assert_eq!(index.at("eur", between), None);
    }
}
