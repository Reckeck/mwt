//! Properties of the key-rate merge.

use proptest::prelude::*;

use super::interest::merge_key_rates;
use crate::model::deposit::KeyRatePoint;
use crate::testkit::props::amount;

fn points() -> impl Strategy<Value = Vec<KeyRatePoint>> {
    prop::collection::vec((1..=28u32, 1..=12u32, amount(1..=3000, 2)), 0..12).prop_map(|rows| {
        rows.into_iter()
            .map(|(day, month, rate)| KeyRatePoint {
                date: format!("2024-{month:02}-{day:02}"),
                rate,
            })
            .collect()
    })
}

proptest! {
    #[test]
    fn merged_key_rates_are_sorted_and_deduplicated(known in points(), fresh in points()) {
        let merged = merge_key_rates(&known, &fresh);
        prop_assert!(merged.windows(2).all(|pair| pair[0].date < pair[1].date));
        for point in known.iter().chain(&fresh) {
            prop_assert!(merged.iter().any(|row| row.date == point.date));
        }
    }

    #[test]
    fn the_fresh_key_rate_wins(known in points(), fresh in points()) {
        let merged = merge_key_rates(&known, &fresh);
        // The last fresh point of a day is the one that is kept.
        for point in fresh.iter().rev() {
            let kept = merged.iter().find(|row| row.date == point.date).unwrap();
            let last = fresh.iter().rev().find(|row| row.date == point.date).unwrap();
            prop_assert_eq!(kept.rate, last.rate);
        }
    }

    #[test]
    fn merging_key_rates_is_idempotent(known in points(), fresh in points()) {
        let merged = merge_key_rates(&known, &fresh);
        prop_assert_eq!(merge_key_rates(&merged, &fresh), merged.clone());
        prop_assert_eq!(merge_key_rates(&merged, &[]), merged);
    }
}
