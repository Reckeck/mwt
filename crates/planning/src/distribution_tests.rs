use mwt_base::Amount;
use mwt_model::{DistKind, DistTarget, DistributionPart, DistributionRow, EffectRef, PartStatus};
use proptest::prelude::*;
use rust_decimal::Decimal;

use super::{plan_distribution, resolve_rows};

fn amount(value: i64) -> Amount {
    Amount::new(Decimal::from(value))
}

fn row(kind: DistKind, target: DistTarget) -> DistributionRow {
    DistributionRow { kind, target }
}

fn store(id: &str) -> DistTarget {
    DistTarget::Store { id: id.into() }
}

fn envelope(id: &str) -> DistTarget {
    DistTarget::Envelope { id: id.into() }
}

fn a2_rows(store_sum: i64) -> Vec<DistributionRow> {
    vec![
        row(
            DistKind::Sum {
                amount: amount(store_sum),
            },
            store("deposit"),
        ),
        row(DistKind::Percent { pct: amount(5) }, envelope("trip")),
        row(
            DistKind::Sum {
                amount: amount(3_000),
            },
            envelope("tax"),
        ),
        row(DistKind::Remainder, DistTarget::None),
    ]
}

fn amounts(parts: &[DistributionPart]) -> Vec<Amount> {
    parts.iter().map(|part| part.amount).collect()
}

fn with_effects(mut parts: Vec<DistributionPart>) -> Vec<DistributionPart> {
    for (index, part) in parts.iter_mut().enumerate() {
        if !matches!(part.kind, DistKind::Remainder) {
            part.effect = Some(EffectRef::Transfer {
                tx_id: format!("tx{index}"),
            });
        }
    }
    parts
}

#[test]
fn a2_numbers_resolve() {
    let resolved = resolve_rows(amount(120_000), &a2_rows(20_000), 2).unwrap();
    assert_eq!(resolved.over, None);
    assert_eq!(
        amounts(&resolved.parts),
        vec![amount(20_000), amount(6_000), amount(3_000), amount(91_000)]
    );
    assert!(
        resolved
            .parts
            .iter()
            .all(|part| part.status == PartStatus::Done)
    );
}

#[test]
fn percent_rounds_to_the_minor_units() {
    let rows = vec![
        row(DistKind::Percent { pct: amount(1) }, envelope("trip")),
        row(DistKind::Remainder, DistTarget::None),
    ];
    let resolved = resolve_rows(Amount::parse("100.55").unwrap(), &rows, 2).unwrap();
    assert_eq!(resolved.parts[0].amount, Amount::parse("1.01").unwrap());
    assert_eq!(resolved.parts[1].amount, Amount::parse("99.54").unwrap());
}

#[test]
fn over_allocation_is_reported_and_the_remainder_stays_zero() {
    let resolved = resolve_rows(amount(120_000), &a2_rows(130_000), 2).unwrap();
    assert_eq!(resolved.over, Some(amount(19_000)));
    assert_eq!(resolved.parts[3].amount, Amount::ZERO);
}

#[test]
fn exactly_one_remainder_is_required() {
    let none = vec![row(DistKind::Sum { amount: amount(1) }, store("a"))];
    assert!(resolve_rows(amount(10), &none, 2).is_err());
    let two = vec![
        row(DistKind::Remainder, DistTarget::None),
        row(DistKind::Remainder, DistTarget::None),
    ];
    assert!(resolve_rows(amount(10), &two, 2).is_err());
}

#[test]
fn editing_a_row_removes_the_old_effect_and_creates_one() {
    let old = with_effects(
        resolve_rows(amount(120_000), &a2_rows(20_000), 2)
            .unwrap()
            .parts,
    );
    let new = resolve_rows(amount(120_000), &a2_rows(25_000), 2)
        .unwrap()
        .parts;
    let plan = plan_distribution(&old, &new);
    assert_eq!(
        plan.remove,
        vec![EffectRef::Transfer {
            tx_id: "tx0".into()
        }]
    );
    assert_eq!(plan.create.len(), 1);
    assert_eq!(plan.create[0].amount, amount(25_000));
    assert_eq!(plan.parts[1].effect, old[1].effect);
    assert_eq!(plan.parts[2].effect, old[2].effect);
}

#[test]
fn an_unchanged_distribution_changes_nothing() {
    let old = with_effects(
        resolve_rows(amount(120_000), &a2_rows(20_000), 2)
            .unwrap()
            .parts,
    );
    let new = resolve_rows(amount(120_000), &a2_rows(20_000), 2)
        .unwrap()
        .parts;
    let plan = plan_distribution(&old, &new);
    assert!(plan.create.is_empty() && plan.remove.is_empty());
    assert_eq!(plan.parts, old);
}

#[test]
fn undo_all_removes_every_effect() {
    let old = with_effects(
        resolve_rows(amount(120_000), &a2_rows(20_000), 2)
            .unwrap()
            .parts,
    );
    let plan = plan_distribution(&old, &[]);
    assert_eq!(plan.remove.len(), 3);
    assert!(plan.create.is_empty() && plan.parts.is_empty());
}

proptest! {
    #[test]
    fn the_parts_always_add_up_to_the_payout(
        payout in 1..10_000_000i64,
        sums in proptest::collection::vec(1..1_000_000i64, 0..4),
        pcts in proptest::collection::vec(1..40i64, 0..3),
    ) {
        let mut rows: Vec<DistributionRow> = sums
            .iter()
            .map(|value| row(DistKind::Sum { amount: amount(*value) }, store("s")))
            .collect();
        rows.extend(pcts.iter().map(|pct| row(DistKind::Percent { pct: amount(*pct) }, envelope("e"))));
        rows.push(row(DistKind::Remainder, DistTarget::None));
        let payout = Amount::new(Decimal::new(payout, 2));
        let resolved = resolve_rows(payout, &rows, 2).unwrap();
        let total = resolved.parts.iter().fold(Amount::ZERO, |sum, part| sum + part.amount);
        prop_assert!(!resolved.parts.iter().any(|part| part.amount.is_negative()));
        match resolved.over {
            None => prop_assert_eq!(total, payout),
            Some(over) => prop_assert_eq!(total, payout + over),
        }
    }
}
