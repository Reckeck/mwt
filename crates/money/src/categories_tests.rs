//! Tests of the categories area: the old inline tests (names and bodies unchanged),
//! then the deciders on a bare document, the rule pins and the delegate pins.

use super::categories::*;

#[test]
fn slugs_are_ascii_keys() {
    assert_eq!(slug("Coffee & Tea"), "coffee_tea");
    assert_eq!(slug("  Rent 2024 "), "rent_2024");
    assert!(slug("Кофе").starts_with("cat_"));
}
