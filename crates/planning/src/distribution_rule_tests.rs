use mwt_base::Amount;
use mwt_model::{DistKind, DistTarget, DistributionRow, DistributionRule, OnOtherAmount};
use rust_decimal::Decimal;

use super::resolve_distribution;

fn amount(raw: &str) -> Amount {
    Amount::parse(raw).unwrap()
}

fn row(kind: DistKind, target: DistTarget) -> DistributionRow {
    DistributionRow { kind, target }
}

fn rule(on_other_amount: OnOtherAmount, floor: Option<&str>) -> DistributionRule {
    DistributionRule {
        rows: vec![
            row(
                DistKind::Sum {
                    amount: amount("20000"),
                },
                DistTarget::Store {
                    id: "deposit".into(),
                },
            ),
            row(
                DistKind::Percent {
                    pct: Amount::new(Decimal::from(5)),
                },
                DistTarget::Envelope { id: "trip".into() },
            ),
            row(
                DistKind::Sum {
                    amount: amount("3000"),
                },
                DistTarget::Envelope { id: "tax".into() },
            ),
            row(DistKind::Remainder, DistTarget::None),
        ],
        on_other_amount,
        keep_on_card_min: floor.map(amount),
    }
}

fn amounts(rule: &DistributionRule, actual: &str, card: &str) -> Vec<Amount> {
    resolve_distribution(rule, amount(actual), amount("120000"), amount(card), 2)
        .unwrap()
        .rows
        .iter()
        .map(|resolved| resolved.amount)
        .collect()
}

/// A3: shrink_remainder keeps sum rows, the remainder absorbs the difference.
#[test]
fn a3_shrink_remainder() {
    let rule = rule(OnOtherAmount::ShrinkRemainder, None);
    let resolved =
        resolve_distribution(&rule, amount("118500"), amount("120000"), amount("0"), 2).unwrap();
    assert_eq!(
        amounts(&rule, "118500", "0"),
        [
            amount("20000"),
            amount("5925"),
            amount("3000"),
            amount("89575")
        ]
    );
    assert_eq!(resolved.remainder, amount("89575"));
}

/// A3: shrink_all scales the sum rows.
#[test]
fn a3_shrink_all() {
    let rule = rule(OnOtherAmount::ShrinkAll, None);
    let got = amounts(&rule, "118500", "0");
    assert_eq!(
        got,
        [
            amount("19750"),
            amount("5925"),
            amount("2962.50"),
            amount("89862.50")
        ]
    );
    assert_eq!(
        got.iter().fold(Amount::ZERO, |sum, one| sum + *one),
        amount("118500")
    );
}

/// A3: ask resolves nothing.
#[test]
fn a3_ask() {
    let rule = rule(OnOtherAmount::Ask, None);
    let resolved =
        resolve_distribution(&rule, amount("118500"), amount("120000"), amount("0"), 2).unwrap();
    assert!(resolved.ask);
    assert!(resolved.rows.is_empty());
}

/// An equal payout resolves even under `ask`.
#[test]
fn equal_payout_resolves_under_ask() {
    let rule = rule(OnOtherAmount::Ask, None);
    let resolved =
        resolve_distribution(&rule, amount("120000"), amount("120000"), amount("0"), 2).unwrap();
    assert!(!resolved.ask);
    assert_eq!(resolved.remainder, amount("90000") + amount("1000"));
}

/// Q11: fixed and percent rows above the payout are `over`.
#[test]
fn q11_over() {
    let rule = rule(OnOtherAmount::ShrinkRemainder, None);
    let resolved =
        resolve_distribution(&rule, amount("20000"), amount("120000"), amount("0"), 2).unwrap();
    assert_eq!(resolved.over, Some(amount("4000")));
    assert!(resolved.rows.is_empty());
}

/// A7: rows are cut from the bottom until the card holds the floor; the balance is the one before
/// the rows (29400 - 29000 asked = 400 left).
#[test]
fn a7_cut_from_the_bottom() {
    let rule = rule(OnOtherAmount::ShrinkRemainder, Some("1000"));
    let resolved = resolve_distribution(
        &rule,
        amount("120000"),
        amount("120000"),
        amount("29400"),
        2,
    )
    .unwrap();
    assert_eq!(resolved.cut, [(2, amount("600"))]);
    assert_eq!(resolved.rows[2].amount, amount("2400"));
    let deep = resolve_distribution(
        &rule,
        amount("120000"),
        amount("120000"),
        amount("25500"),
        2,
    )
    .unwrap();
    assert_eq!(deep.cut, [(2, amount("3000")), (1, amount("1500"))]);
    assert_eq!(deep.rows[1].amount, amount("4500"));
}

/// A7: an unreachable floor zeroes every row.
#[test]
fn a7_unreachable_floor() {
    let rule = rule(OnOtherAmount::ShrinkRemainder, Some("1000000"));
    let resolved =
        resolve_distribution(&rule, amount("120000"), amount("120000"), amount("0"), 2).unwrap();
    assert!(resolved.rows[..3].iter().all(|one| one.amount.is_zero()));
    assert_eq!(resolved.remainder, amount("120000"));
}
