//! Properties of the split: rounding helpers and the books of an event.

use proptest::prelude::*;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

use super::split::{compute, round_places, round_to_step, spread};
use mwt_base::Amount;
use mwt_model::{EventItem, ItemSplit, ME, Participant, PriceMode, Purchase, SharedEvent};
use mwt_testkit::props::{amount, money};

fn thousandths() -> impl Strategy<Value = Amount> {
    amount(-100_000_000_000..=100_000_000_000, 3)
}

fn step() -> impl Strategy<Value = Amount> {
    amount(1..=100_000, 2)
}

proptest! {
    #[test]
    fn spread_parts_sum_to_the_cost(
        cost in money(),
        weights in prop::collection::vec(amount(1..=1000, 1), 1..8),
        taker in 0..10usize,
    ) {
        let weights: Vec<(String, Amount)> = weights
            .into_iter()
            .enumerate()
            .map(|(index, weight)| (format!("p{index}"), weight))
            .collect();
        let parts = spread(cost, &weights, &format!("p{taker}"));
        prop_assert_eq!(parts.len(), weights.len());
        let sum = parts.values().fold(Amount::ZERO, |sum, part| sum + *part);
        prop_assert_eq!(sum, cost);
    }

    #[test]
    fn round_places_is_idempotent_with_error_up_to_half_a_unit(
        value in thousandths(),
        places in 0..4u32,
    ) {
        let once = round_places(value, places);
        prop_assert_eq!(round_places(once, places), once);
        let half = Amount::new(Decimal::new(5, places + 1));
        prop_assert!((once - value).abs() <= half);
    }

    #[test]
    fn round_to_step_is_idempotent_with_error_up_to_half_a_step(
        value in thousandths(),
        step in step(),
    ) {
        let once = round_to_step(value, step);
        prop_assert_eq!(round_to_step(once, step), once);
        prop_assert!((once - value).abs() <= step / Amount::from(2));
    }
}

fn buyer_of(index: usize, people: usize) -> String {
    if index.is_multiple_of(people) {
        ME.to_string()
    } else {
        format!("p{}", index % people)
    }
}

/// An event of `people` (me included) with items bought by various people.
fn event() -> impl Strategy<Value = SharedEvent> {
    let item = (
        prop::collection::vec((money(), 0..8usize), 0..3),
        prop::collection::vec(any::<bool>(), 0..5),
        any::<bool>(),
    );
    (
        2..5usize,
        any::<bool>(),
        prop::sample::select(vec!["0.01", "1", "5", "10"]),
        prop::collection::vec(item, 1..5),
    )
        .prop_map(|(people, i_participate, step, items)| {
            let items = items
                .into_iter()
                .enumerate()
                .map(|(index, (purchases, among, by_units))| EventItem {
                    id: format!("i{index}"),
                    purchases: purchases
                        .into_iter()
                        .enumerate()
                        .map(|(number, (total, buyer))| Purchase {
                            id: format!("i{index}-{number}"),
                            buyer: buyer_of(buyer, people),
                            qty: Amount::from(1),
                            amount: total,
                            currency: "rub".into(),
                            price_mode: PriceMode::Total,
                            rate_to_event: Amount::from(1),
                            at: "2026-11-02T12:00:00Z".into(),
                            ..Purchase::default()
                        })
                        .collect(),
                    split: if by_units {
                        ItemSplit::ByUnits {
                            units: among
                                .iter()
                                .enumerate()
                                .map(|(who, on)| {
                                    (buyer_of(who, people), Amount::from(u32::from(*on) + 1))
                                })
                                .collect::<BTreeMap<_, _>>(),
                        }
                    } else {
                        ItemSplit::Equal {
                            among: among
                                .iter()
                                .enumerate()
                                .filter(|(_, on)| **on)
                                .map(|(who, _)| buyer_of(who, people))
                                .collect(),
                        }
                    },
                    ..EventItem::default()
                })
                .collect();
            SharedEvent {
                id: "event".into(),
                currency: "rub".into(),
                rounding_step: Amount::parse(step).unwrap(),
                organizer: ME.into(),
                i_participate,
                participants: (1..people)
                    .map(|index| Participant {
                        id: format!("p{index}"),
                        name: format!("Person {index}"),
                        ..Participant::default()
                    })
                    .collect(),
                items,
                ..SharedEvent::default()
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn compute_shares_sum_to_spent_and_saldos_to_zero(event in event()) {
        let split = compute(&event);
        let shares = split.people.iter().fold(Amount::ZERO, |sum, row| sum + row.share);
        prop_assert_eq!(shares, split.spent);
        let saldos = split.people.iter().fold(Amount::ZERO, |sum, row| sum + row.saldo);
        prop_assert_eq!(saldos, Amount::ZERO);
    }
}
