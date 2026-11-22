//! Properties of the top-up split: the parts add up to the amount.

use proptest::prelude::*;

use super::allocation::split_topup;
use mwt_base::Amount;
use mwt_model::AssetClass;
use mwt_testkit::props::{amount, money};

fn classes_with_values() -> impl Strategy<Value = (Vec<AssetClass>, Vec<Amount>)> {
    prop::collection::vec((1..=100u32, any::<bool>(), money()), 1..6).prop_map(|rows| {
        let mut classes = Vec::new();
        let mut values = Vec::new();
        for (index, (target, cap, value)) in rows.into_iter().enumerate() {
            classes.push(AssetClass {
                id: format!("c{index}"),
                name: format!("class {index}"),
                target_pct: Amount::from(target),
                // The first basket is never capped, so some weight is always positive.
                cap: cap && index > 0,
            });
            values.push(value);
        }
        (classes, values)
    })
}

proptest! {
    #[test]
    fn split_topup_sums_to_the_amount_to_the_cent(
        (classes, values) in classes_with_values(),
        top_up in amount(0..=100_000_000, 3),
    ) {
        let parts = split_topup(&classes, &values, top_up);
        prop_assert_eq!(parts.len(), classes.len());
        let sum = parts.iter().fold(Amount::ZERO, |sum, (part, _)| sum + *part);
        prop_assert_eq!(sum, top_up.round(2));
    }
}
