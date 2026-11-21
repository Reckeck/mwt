//! Strategies shared by the property tests (`*_props.rs` next to the code).

use mwt_base::Amount;
use proptest::prelude::*;
use rust_decimal::Decimal;

/// An amount of `scale` decimals from an integer in `range`.
pub(crate) fn amount(
    range: std::ops::RangeInclusive<i64>,
    scale: u32,
) -> impl Strategy<Value = Amount> {
    range.prop_map(move |raw| Amount::new(Decimal::new(raw, scale)))
}

/// An RFC 3339 timestamp of `secs` since the epoch.
pub(crate) fn iso(secs: i64) -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp(secs)
        .expect("in range")
        .format(&Rfc3339)
        .expect("formats")
}
