//! Properties of `Amount`.

use proptest::prelude::*;

use crate::Amount;
use crate::props::{any_amount, exact_amount, input_amount};

proptest! {
    #[test]
    fn parse_never_panics(raw in "\\PC{0,40}") {
        let _ = Amount::parse(&raw);
    }

    #[test]
    fn parse_of_format_is_the_value(value in input_amount()) {
        prop_assert!(value.within_input_bound());
        prop_assert_eq!(Amount::parse(&value.to_string()).unwrap(), value);
    }

    #[test]
    fn checked_add_and_sub_are_inverse(left in exact_amount(), right in exact_amount()) {
        if let Some(sum) = left.checked_add(right) {
            prop_assert_eq!(sum.checked_sub(right), Some(left));
        }
        if let Some(diff) = left.checked_sub(right) {
            prop_assert_eq!(diff.checked_add(right), Some(left));
        }
    }

    #[test]
    fn checked_ops_never_panic(left in any_amount(), right in any_amount()) {
        let _ = left.checked_add(right);
        let _ = left.checked_sub(right);
        let _ = left.checked_mul(right);
        let _ = left.checked_div(right);
    }

    #[test]
    fn abs_is_non_negative_and_idempotent(value in input_amount()) {
        prop_assert!(!value.abs().is_negative());
        prop_assert_eq!(value.abs().abs(), value.abs());
    }

    #[test]
    fn exactly_one_sign_predicate_holds(value in any_amount()) {
        let held = [value.is_zero(), value.is_negative(), value.is_positive()];
        prop_assert_eq!(held.iter().filter(|flag| **flag).count(), 1);
    }

    #[test]
    fn serde_json_round_trips(value in any_amount()) {
        let json = serde_json::to_string(&value).unwrap();
        prop_assert_eq!(serde_json::from_str::<Amount>(&json).unwrap(), value);
    }
}
