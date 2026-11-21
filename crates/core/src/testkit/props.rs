//! Strategies shared by the property tests (`*_props.rs` next to the code).

use chrono::NaiveDate;
use proptest::prelude::*;
use rust_decimal::Decimal;

use crate::Amount;

/// An amount of `scale` decimals from an integer in `range`.
pub fn amount(range: std::ops::RangeInclusive<i64>, scale: u32) -> impl Strategy<Value = Amount> {
    range.prop_map(move |raw| Amount::new(Decimal::new(raw, scale)))
}

/// A non-negative money amount to the cent, up to ten million.
pub fn money() -> impl Strategy<Value = Amount> {
    amount(0..=1_000_000_000, 2)
}

/// Any day from 1970 to about 2134.
pub fn day() -> impl Strategy<Value = NaiveDate> {
    (0..60_000i64).prop_map(|offset| {
        NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch") + chrono::Duration::days(offset)
    })
}

/// An RFC 3339 timestamp of `secs` since the epoch.
pub fn iso(secs: i64) -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp(secs)
        .expect("in range")
        .format(&Rfc3339)
        .expect("formats")
}
