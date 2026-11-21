//! Properties of the calculator preview.

use proptest::prelude::*;

use super::{CalcInput, CalcMode, preview};
use crate::Amount;
use crate::props::input_amount;

fn mode() -> impl Strategy<Value = CalcMode> {
    prop_oneof![Just(CalcMode::Converter), Just(CalcMode::Position)]
}

fn calc_input() -> impl Strategy<Value = CalcInput> {
    (
        mode(),
        (input_amount(), input_amount(), input_amount()),
        (
            proptest::option::of(input_amount()),
            proptest::option::of(input_amount()),
            input_amount(),
        ),
    )
        .prop_map(
            |(mode, (amount, price_in, price_out), (target_price, stop_price, fee_pct))| {
                CalcInput {
                    mode,
                    amount,
                    price_in,
                    price_out,
                    target_price,
                    stop_price,
                    fee_pct,
                    ..CalcInput::default()
                }
            },
        )
}

proptest! {
    #[test]
    fn preview_never_panics_and_is_deterministic(input in calc_input()) {
        prop_assert_eq!(preview(&input), preview(&input));
    }

    #[test]
    fn preview_of_zero_amount_is_empty(input in calc_input()) {
        let input = CalcInput { amount: Amount::ZERO, ..input };
        prop_assert_eq!(preview(&input), Default::default());
    }
}
