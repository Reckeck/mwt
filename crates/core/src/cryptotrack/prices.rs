//! Historic prices of crypto assets, kept in the document's `crypto_prices`
//! (spec E4).
//!
//! They live apart from `price_cache`: that collection thins every series and
//! caps it at `HISTORY_CAP` points, which would eat a history asked for one
//! date at a time, and one sync rule covers all of it. Here a point is
//! written once per `(token key, date)` and never thinned.
//!
//! No price is `None` plus a staleness mark, never zero. Fetching is in
//! [`crate::pricing::llama`] and does not touch the document; the functions
//! here apply its answer and read the cache back.

use chrono::NaiveDate;

use crate::Amount;
use crate::error::Result;
use crate::model::cryptotrack::{CryptoAsset, CryptoPricePoint};
use crate::model::vault::VaultDoc;

/// The moment a day's price is stored under: midnight UTC.
pub fn date_ts(date: NaiveDate) -> String {
    format!("{}T00:00:00Z", date.format("%Y-%m-%d"))
}

/// Unix seconds of midnight UTC of `date`.
pub fn date_secs(date: NaiveDate) -> i64 {
    date.and_hms_opt(0, 0, 0)
        .map_or(0, |moment| moment.and_utc().timestamp())
}

/// What the cache knows about one price.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PriceLookup {
    /// `None` when nothing usable is cached; never zero.
    pub value: Option<Amount>,
    /// The `ts` of the point the value comes from.
    pub at: Option<String>,
    /// The value is not the price of the asked day: an older point stands in
    /// for it, or there is none. A UI shows it as "outdated".
    pub stale: bool,
}

/// Insert fetched points. A point of the same `(token key, ts)` is replaced;
/// a zero price is never stored.
pub fn apply_historic(doc: &mut VaultDoc, fresh: &[CryptoPricePoint]) {
    for point in fresh.iter().filter(|point| !point.value_usd.is_zero()) {
        match doc
            .crypto_prices
            .iter_mut()
            .find(|have| have.token_key == point.token_key && have.ts == point.ts)
        {
            Some(have) => *have = point.clone(),
            None => doc.crypto_prices.push(point.clone()),
        }
    }
}

/// The cached price of `asset` for `date`: the point of that day, else the
/// newest older one marked stale, else nothing (stale).
pub fn lookup(doc: &VaultDoc, asset: &CryptoAsset, date: NaiveDate) -> PriceLookup {
    let key = asset.key();
    let wanted = date_ts(date);
    let best = doc
        .crypto_prices
        .iter()
        .filter(|point| point.token_key == key && !point.value_usd.is_zero() && point.ts <= wanted)
        .max_by(|left, right| left.ts.cmp(&right.ts));
    match best {
        Some(point) => PriceLookup {
            value: Some(point.value_usd),
            at: Some(point.ts.clone()),
            stale: point.ts != wanted,
        },
        None => PriceLookup {
            value: None,
            at: None,
            stale: true,
        },
    }
}

/// Apply a fetch outcome and answer from the cache. A failed fetch (offline,
/// a provider outage) is not an error of the calculation: the cache answers,
/// marked stale unless it holds the very day.
pub fn resolve(
    doc: &mut VaultDoc,
    asset: &CryptoAsset,
    date: NaiveDate,
    fetched: Result<Vec<CryptoPricePoint>>,
) -> PriceLookup {
    if let Ok(points) = fetched {
        apply_historic(doc, &points);
    }
    lookup(doc, asset, date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::TokenId;

    fn asset() -> CryptoAsset {
        CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xabc"))
    }

    fn day(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, day).unwrap()
    }

    fn point(date: NaiveDate, price: &str) -> CryptoPricePoint {
        CryptoPricePoint {
            token_key: asset().key(),
            value_usd: Amount::parse(price).unwrap(),
            ts: date_ts(date),
            source: "defillama".into(),
        }
    }

    #[test]
    fn history_goes_to_crypto_prices_not_to_price_cache() {
        let mut doc = VaultDoc::default();
        apply_historic(&mut doc, &[point(day(1), "2")]);
        assert_eq!(doc.crypto_prices.len(), 1);
        assert!(doc.price_cache.is_empty());
    }

    #[test]
    fn history_is_not_thinned_by_the_price_cache_cap() {
        let mut doc = VaultDoc::default();
        let start = NaiveDate::from_ymd_opt(2015, 1, 1).unwrap();
        let points: Vec<_> = (0..2500)
            .map(|offset| point(start + chrono::Days::new(offset), "1"))
            .collect();
        apply_historic(&mut doc, &points);
        assert_eq!(doc.crypto_prices.len(), 2500);
    }

    #[test]
    fn the_same_day_is_replaced_not_duplicated() {
        let mut doc = VaultDoc::default();
        apply_historic(&mut doc, &[point(day(1), "2")]);
        apply_historic(&mut doc, &[point(day(1), "3")]);
        assert_eq!(doc.crypto_prices.len(), 1);
        assert_eq!(
            lookup(&doc, &asset(), day(1)).value,
            Amount::parse("3").ok()
        );
    }

    #[test]
    fn no_price_is_none_and_stale_never_zero() {
        let doc = VaultDoc::default();
        let found = lookup(&doc, &asset(), day(1));
        assert_eq!(found.value, None);
        assert!(found.stale);
        let mut doc = VaultDoc::default();
        apply_historic(&mut doc, &[point(day(1), "0")]);
        assert!(doc.crypto_prices.is_empty());
    }

    #[test]
    fn an_older_point_stands_in_marked_stale() {
        let mut doc = VaultDoc::default();
        apply_historic(&mut doc, &[point(day(1), "2")]);
        let exact = lookup(&doc, &asset(), day(1));
        assert!(!exact.stale);
        let later = lookup(&doc, &asset(), day(5));
        assert_eq!(later.value, Amount::parse("2").ok());
        assert!(later.stale);
        // Nothing from the future stands in for the past.
        assert_eq!(
            lookup(&doc, &asset(), NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()).value,
            None
        );
    }

    #[test]
    fn offline_yields_the_cache_with_a_staleness_mark() {
        let mut doc = VaultDoc::default();
        apply_historic(&mut doc, &[point(day(1), "2")]);
        let offline = Err(Error::Network("offline".into()));
        let found = resolve(&mut doc, &asset(), day(9), offline);
        assert_eq!(found.value, Amount::parse("2").ok());
        assert!(found.stale);
        let nothing = resolve(
            &mut VaultDoc::default(),
            &asset(),
            day(9),
            Err(Error::Network("offline".into())),
        );
        assert_eq!(nothing.value, None);
        assert!(nothing.stale);
    }

    #[test]
    fn a_fetched_answer_is_applied() {
        let mut doc = VaultDoc::default();
        let found = resolve(&mut doc, &asset(), day(2), Ok(vec![point(day(2), "4")]));
        assert_eq!(found.value, Amount::parse("4").ok());
        assert!(!found.stale);
    }
}
