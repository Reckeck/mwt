//! Tests of the interest area: the old inline tests (names kept), which drive
//! the same rules through the `UnlockedVault` delegates.

use super::interest::*;
use crate::Amount;
use crate::model::deposit::Capitalization;

fn amount_str(raw: &str) -> Amount {
    Amount::parse(raw).unwrap()
}

/// `grow` keeps the numbers the old f64 formula gave (1000 at 12 % over a year).
#[test]
fn grow_pins_the_yearly_figures_of_each_capitalization() {
    let grown = |days: i64, capitalization: Capitalization| {
        grow(amount_str("1000"), amount_str("12"), days, capitalization)
    };
    assert_eq!(grown(365, Capitalization::None), amount_str("1120.00"));
    assert_eq!(grown(365, Capitalization::AtEnd), amount_str("1120.00"));
    assert_eq!(grown(365, Capitalization::Daily), amount_str("1127.47"));
    assert_eq!(grown(365, Capitalization::Monthly), amount_str("1126.83"));
    // Two years, monthly: 1000 * 1.01^24.
    assert_eq!(grown(730, Capitalization::Monthly), amount_str("1269.73"));
}
