//! Tests of the valuation area: the old inline tests (names kept), which drive
//! the same rules through the `UnlockedVault` delegates.

use crate::Amount;
use crate::money::valuation::*;
use crate::service::test_support::*;

const DAY: i64 = 86_400;

fn curve(points: &[(i64, &str)], rule: Option<(&str, Option<&str>)>) -> ValuationCurve {
    ValuationCurve {
        currency: "usd".into(),
        points: points
            .iter()
            .map(|(at, price)| (*at, amount(price)))
            .collect(),
        rule: rule.map(|(pct, floor)| CurveRule {
            monthly: (Amount::from(1) + amount(pct) / hundred()).root_n(12),
            floor: floor.map(amount),
        }),
    }
}

fn near(left: Amount, right: &str) -> bool {
    (left - amount(right)).abs() < amount("0.01")
}

#[test]
fn a_huge_value_with_a_growth_rule_saturates() {
    let huge = curve(&[(0, "1000000000000000000")], Some(("100", None)));
    let value = huge.value_at(MAX_MONTHS * MONTH_SECS).unwrap();
    assert!(value > amount("1000000000000000000"));
    assert!(value <= Amount::MAX);
}

#[test]
fn points_join_by_straight_lines() {
    let line = curve(&[(0, "100"), (10 * DAY, "200")], None);
    assert_eq!(line.value_at(-DAY), Some(amount("100")));
    assert!(near(line.value_at(5 * DAY).unwrap(), "150"));
    assert_eq!(line.value_at(20 * DAY), Some(amount("200")));
}

#[test]
fn the_rule_runs_after_the_last_point() {
    let grow = curve(&[(0, "1000")], Some(("12", None)));
    let year = grow.value_at(12 * MONTH_SECS).unwrap();
    assert!(near(year, "1120"), "{year}");
    let half = grow.value_at(6 * MONTH_SECS).unwrap();
    assert!(half > amount("1050") && half < amount("1060"), "{half}");
}

#[test]
fn the_floor_stops_depreciation() {
    let car = curve(&[(0, "1000")], Some(("-50", Some("400"))));
    let late = car.value_at(10 * 12 * MONTH_SECS).unwrap();
    assert_eq!(late, amount("400"));
    // A valuation below the floor is kept, not lifted.
    let below = curve(&[(0, "300")], Some(("-10", Some("400"))));
    assert_eq!(below.value_at(24 * MONTH_SECS), Some(amount("300")));
}
