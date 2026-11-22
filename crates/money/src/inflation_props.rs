//! Properties of the inflation merge.

use proptest::prelude::*;

use super::inflation::merge_readings;
use mwt_model::{InflationReading, InflationSource};
use mwt_testkit::props::amount;

fn readings(currencies: &'static [&'static str]) -> impl Strategy<Value = Vec<InflationReading>> {
    prop::collection::vec(
        (
            prop::sample::select(currencies),
            1..=12u32,
            2023..=2025u32,
            amount(-500..=3000, 1),
        ),
        0..14,
    )
    .prop_map(|rows| {
        rows.into_iter()
            .map(|(currency, month, year, yoy_pct)| InflationReading {
                currency: currency.into(),
                month: format!("{year}-{month:02}"),
                yoy_pct,
                source: InflationSource::Cbr,
            })
            .collect()
    })
}

proptest! {
    #[test]
    fn merged_readings_are_sorted_and_deduplicated(
        known in readings(&["rub", "usd"]),
        fresh in readings(&["rub", "usd", "RUB", " eur "]),
    ) {
        let (merged, _) = merge_readings(known, &fresh);
        let keys: Vec<(&str, &str)> = merged
            .iter()
            .map(|row| (row.currency.as_str(), row.month.as_str()))
            .collect();
        prop_assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn merging_readings_is_idempotent_and_the_second_pass_has_no_news(
        known in readings(&["rub", "usd"]),
        fresh in readings(&["rub", "usd", "RUB", " eur "]),
    ) {
        let (merged, _) = merge_readings(known, &fresh);
        let (again, news) = merge_readings(merged.clone(), &fresh);
        prop_assert_eq!(again, merged);
        prop_assert!(news.is_empty());
    }
}
