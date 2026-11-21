//! Strategies shared by the property tests (`*_props.rs` next to the code).

use chrono::NaiveDate;
use chrono_tz::TZ_VARIANTS;
use proptest::prelude::*;
use rust_decimal::Decimal;

use crate::Amount;

/// An amount within `Amount::MAX_INPUT`, up to 18 decimals.
pub(crate) fn input_amount() -> impl Strategy<Value = Amount> {
    const BOUND: i64 = 1_000_000_000_000_000_000;
    (-BOUND..=BOUND, 0..=18u32).prop_map(|(raw, scale)| Amount::new(Decimal::new(raw, scale)))
}

/// An amount within `Amount::MAX_INPUT` with at most 8 decimals: the sum of two
/// such amounts still fits the 96-bit mantissa, so no scale is rounded away.
pub(crate) fn exact_amount() -> impl Strategy<Value = Amount> {
    const BOUND: i64 = 1_000_000_000_000_000_000;
    (-BOUND..=BOUND, 0..=8u32).prop_map(|(raw, scale)| Amount::new(Decimal::new(raw, scale)))
}

/// Any representable amount, `Decimal::MAX` and `MIN` included.
pub(crate) fn any_amount() -> impl Strategy<Value = Amount> {
    prop_oneof![
        1 => Just(Amount::MAX),
        1 => Just(Amount::MIN),
        8 => (any::<u32>(), any::<u32>(), any::<u32>(), any::<bool>(), 0..=28u32)
            .prop_map(|(lo, mid, hi, negative, scale)| {
                Amount::new(Decimal::from_parts(lo, mid, hi, negative, scale))
            }),
    ]
}

/// A day from 1970 to 2200.
pub(crate) fn day() -> impl Strategy<Value = NaiveDate> {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch");
    let last = NaiveDate::from_ymd_opt(2200, 12, 31).expect("end");
    (0..=(last - epoch).num_days()).prop_map(move |offset| epoch + chrono::Duration::days(offset))
}

/// The name of any IANA zone chrono-tz knows.
pub(crate) fn zone() -> impl Strategy<Value = &'static str> {
    (0..TZ_VARIANTS.len()).prop_map(|index| TZ_VARIANTS[index].name())
}
