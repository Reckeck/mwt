//! Properties of the meter bill formula.

use proptest::prelude::*;

use crate::Amount;
use crate::utilities_formula::{FormulaInput, ZoneValues, check, evaluate, zone_key};

fn formula_text() -> impl Strategy<Value = String> {
    prop_oneof![
        "\\PC{0,40}",
        "[0-9a-zа-я_ ().,;+*/<>=!-]{0,40}",
        proptest::collection::vec(
            prop_oneof![
                Just("показание"),
                Just("предыдущее"),
                Just("расход"),
                Just("тариф"),
                Just("current"),
                Just("used"),
                Just("days"),
                Just("min"),
                Just("max"),
                Just("round"),
                Just("if"),
                Just("("),
                Just(")"),
                Just(";"),
                Just("+"),
                Just("-"),
                Just("*"),
                Just("/"),
                Just("<"),
                Just(">="),
                Just("1"),
                Just("2,5"),
                Just(" "),
            ],
            0..=12,
        )
        .prop_map(|parts| parts.concat()),
    ]
}

fn full_input() -> impl Strategy<Value = FormulaInput> {
    let small = (-1_000_000i64..=1_000_000, 0..=3u32)
        .prop_map(|(raw, scale)| Amount::new(rust_decimal::Decimal::new(raw, scale)));
    (small.clone(), small.clone(), small.clone(), small).prop_map(
        |(current, previous, tariff, days)| FormulaInput {
            total: ZoneValues { current, previous },
            tariff,
            days,
            ..FormulaInput::default()
        },
    )
}

proptest! {
    #[test]
    fn check_and_evaluate_never_panic(formula in formula_text(), input in full_input()) {
        let _ = check(&formula, &[]);
        let _ = evaluate(&formula, &input);
    }

    #[test]
    fn evaluate_succeeds_where_check_accepts(
        formula in formula_text().prop_filter("no division", |text| !text.contains('/')),
        input in full_input(),
    ) {
        if check(&formula, &[]).is_ok() {
            prop_assert!(evaluate(&formula, &input).is_ok(), "{formula}");
        }
    }

    #[test]
    fn a_constant_formula_evaluates_to_itself(
        raw in -1_000_000_000i64..=1_000_000_000,
        scale in 0..=2u32,
        input in full_input(),
    ) {
        let number = rust_decimal::Decimal::new(raw, scale);
        let formula = number.abs().to_string();
        let expected = if raw < 0 { -number.abs() } else { number };
        let sign = if raw < 0 { "-" } else { "" };
        let result = evaluate(&format!("{sign}{formula}"), &input).unwrap();
        prop_assert_eq!(result, Amount::new(expected).round(2));
    }

    #[test]
    fn zone_key_is_idempotent(zone in "\\PC{0,30}") {
        let once = zone_key(&zone);
        prop_assert_eq!(zone_key(&once), once);
    }
}
