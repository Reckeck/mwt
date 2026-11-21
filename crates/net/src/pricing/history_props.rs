//! Properties of the FX and price history merge and thinning.

use proptest::prelude::*;

use super::history::{merge_fx, merge_prices, thin_fx, thin_prices, unix_secs};
use crate::props::{amount, iso};
use mwt_model::{FxPoint, PricePoint};

const START: i64 = 1_700_000_000;

fn fx_points() -> impl Strategy<Value = Vec<FxPoint>> {
    prop::collection::vec(
        (
            prop::sample::select(vec!["eur", "rub", "gbp"]),
            0..(40 * 86_400i64),
            amount(1..=1000, 2),
        ),
        0..120,
    )
    .prop_map(|rows| {
        rows.into_iter()
            .map(|(currency, offset, per_usd)| FxPoint {
                currency: currency.into(),
                per_usd,
                ts: iso(START + offset),
                source: "test".into(),
            })
            .collect()
    })
}

fn price_points() -> impl Strategy<Value = Vec<PricePoint>> {
    prop::collection::vec(
        (
            prop::sample::select(vec!["btc", "eth"]),
            0..(40 * 86_400i64),
            amount(1..=1000, 2),
        ),
        0..120,
    )
    .prop_map(|rows| {
        rows.into_iter()
            .map(|(asset, offset, value_usd)| PricePoint {
                asset_key: asset.into(),
                value_usd,
                ts: iso(START + offset),
                source: "test".into(),
                change_24h_pct: None,
            })
            .collect()
    })
}

fn tracked(currency: &str) -> bool {
    currency != "gbp"
}

/// Grouped by series, strictly increasing in time inside a group.
fn ordered(series: &[(String, i64)]) -> bool {
    let mut closed: Vec<&str> = Vec::new();
    for (index, (key, secs)) in series.iter().enumerate() {
        if index == 0 || series[index - 1].0 != *key {
            if closed.contains(&key.as_str()) {
                return false;
            }
            if index > 0 {
                closed.push(&series[index - 1].0);
            }
        } else if series[index - 1].1 >= *secs {
            return false;
        }
    }
    true
}

fn fx_keys(cache: &[FxPoint]) -> Vec<(String, i64)> {
    cache
        .iter()
        .map(|point| (point.currency.clone(), unix_secs(&point.ts).unwrap()))
        .collect()
}

fn price_keys(cache: &[PricePoint]) -> Vec<(String, i64)> {
    cache
        .iter()
        .map(|point| (point.asset_key.clone(), unix_secs(&point.ts).unwrap()))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn merged_fx_is_sorted_deduplicated_and_idempotent(cache in fx_points(), fresh in fx_points()) {
        let mut merged = cache;
        thin_fx(&mut merged, tracked);
        merge_fx(&mut merged, &fresh, tracked);
        prop_assert!(ordered(&fx_keys(&merged)));
        let once = merged.clone();
        merge_fx(&mut merged, &fresh, tracked);
        prop_assert_eq!(merged, once);
    }

    #[test]
    fn merged_prices_are_sorted_deduplicated_and_idempotent(
        cache in price_points(),
        fresh in price_points(),
    ) {
        let mut merged = cache;
        thin_prices(&mut merged);
        merge_prices(&mut merged, &fresh);
        prop_assert!(ordered(&price_keys(&merged)));
        let once = merged.clone();
        merge_prices(&mut merged, &fresh);
        prop_assert_eq!(merged, once);
    }

    #[test]
    fn thin_fx_is_sorted_deduplicated_and_idempotent(cache in fx_points()) {
        let mut thinned = cache;
        thin_fx(&mut thinned, tracked);
        prop_assert!(ordered(&fx_keys(&thinned)));
        let once = thinned.clone();
        thin_fx(&mut thinned, tracked);
        prop_assert_eq!(thinned, once);
    }

    #[test]
    fn thin_prices_is_sorted_deduplicated_and_idempotent(cache in price_points()) {
        let mut thinned = cache;
        thin_prices(&mut thinned);
        prop_assert!(ordered(&price_keys(&thinned)));
        let once = thinned.clone();
        thin_prices(&mut thinned);
        prop_assert_eq!(thinned, once);
    }
}
