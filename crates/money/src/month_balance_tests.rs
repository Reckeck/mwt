//! Tests of the month_balance area: the old inline tests (names and bodies
//! unchanged), then the carry rule and the delegate pins.

use super::month_balance::*;

#[test]
fn a_carry_month_is_normalized_and_blank_stops_it() {
    assert_eq!(
        parse_carry_since(Some(" 2026-04 ")).unwrap().as_deref(),
        Some("2026-04")
    );
    assert_eq!(parse_carry_since(Some("  ")).unwrap(), None);
    assert_eq!(parse_carry_since(None).unwrap(), None);
    assert!(parse_carry_since(Some("June")).is_err());
}
