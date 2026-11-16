//! The engine's arithmetic: exact, total, and rounded in exactly one place.
//!
//! Every number here comes from a network a stranger can write to, so an
//! operation that a decimal cannot hold returns `None` instead of panicking:
//! `Amount * Amount` and `Amount / Amount` both panic on overflow and on a
//! zero divisor, and a junk token that reports 10^27 units with a real price
//! would take the whole recomputation down (`AGENTS.md`, rule 2: money is
//! `Amount`, and a quantity above what fits is a refusal, not a rounded
//! number).
//!
//! What is rounded, and what is not:
//!
//! * **Quantities are never rounded.** They arrive exact from
//!   [`crate::cryptotrack::sources::evm::Uint256::scaled`] and are added and
//!   subtracted as they came.
//! * **Cost, proceeds and profit are not rounded while they accumulate.** A
//!   decimal holds 28 significant digits; a division rounds only there.
//! * **A disposal removes cost in proportion**
//!   ([`share`]) and never by multiplying a rounded average price, so
//!   disposing of a whole holding removes its whole cost and leaves no
//!   residue behind.
//! * **USD is rounded to cents exactly once**: when it is written into the
//!   document (a snapshot of the capital history). [`usd`] is that rounding,
//!   and it is [`Amount::round`] — the same banker's rounding (a midpoint
//!   goes to the even cent) every other amount in the app is rounded with.
//!   A second rounding mode in the money core would be a bug waiting for a
//!   reconciliation to find it.

use crate::Amount;

/// Places a stored USD value keeps: cents.
pub const USD_PLACES: u32 = 2;

/// `left * right`, or `None` when the product does not fit a decimal.
pub fn mul(left: Amount, right: Amount) -> Option<Amount> {
    left.0.checked_mul(right.0).map(Amount::new)
}

/// `left + right`, or `None` on overflow.
pub fn add(left: Amount, right: Amount) -> Option<Amount> {
    left.0.checked_add(right.0).map(Amount::new)
}

/// `left - right`, or `None` on overflow.
pub fn sub(left: Amount, right: Amount) -> Option<Amount> {
    left.0.checked_sub(right.0).map(Amount::new)
}

/// `left / right`, or `None` when `right` is zero or the quotient does not
/// fit. Rounds only at the 28 significant digits a decimal holds.
pub fn div(left: Amount, right: Amount) -> Option<Amount> {
    if right.is_zero() {
        return None;
    }
    left.0.checked_div(right.0).map(Amount::new)
}

/// The part of `total` that belongs to `part` out of `whole`:
/// `total * part / whole`, `None` when `whole` is zero or nothing fits.
///
/// Used to take cost off a holding, so `part == whole` gives back exactly
/// `total` and a disposal of everything leaves no cost behind.
pub fn share(total: Amount, part: Amount, whole: Amount) -> Option<Amount> {
    if whole.is_zero() {
        return None;
    }
    if part == whole {
        return Some(total);
    }
    match mul(total, part) {
        Some(product) => div(product, whole),
        // Too large to multiply first: divide first and lose only the last
        // digits of the quotient.
        None => mul(div(total, whole)?, part),
    }
}

/// The smaller of two amounts.
pub fn min(left: Amount, right: Amount) -> Amount {
    if left <= right { left } else { right }
}

/// A USD value as it is written down: cents, by the app's own rounding.
pub fn usd(value: Amount) -> Amount {
    value.round(USD_PLACES)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(text: &str) -> Amount {
        Amount::parse(text).expect("a test amount parses")
    }

    #[test]
    fn a_junk_quantity_times_a_price_does_not_panic() {
        // A decimal holds up to 2^96 - 1 (about 7.9 * 10^28), and a junk
        // token routinely reports 10^27 units; at a price of 1000 the product
        // is 10^30 and `Amount * Amount` would panic.
        let huge = amount("1000000000000000000000000000");
        assert_eq!(mul(huge, amount("1000")), None);
        assert_eq!(
            add(amount("79228162514264337593543950335"), amount("1")),
            None
        );
        assert_eq!(
            sub(amount("-79228162514264337593543950335"), amount("1")),
            None
        );
        assert_eq!(
            mul(huge, amount("0.000000001")),
            Some(amount("1000000000000000000"))
        );
    }

    #[test]
    fn dividing_by_zero_is_none_not_a_panic() {
        assert_eq!(div(amount("1"), Amount::ZERO), None);
        assert_eq!(share(amount("10"), amount("1"), Amount::ZERO), None);
        assert_eq!(div(amount("1"), amount("4")), Some(amount("0.25")));
    }

    #[test]
    fn a_whole_share_gives_the_whole_cost_back() {
        // The repeating quotient 100/3 must not leave a residue when the
        // holding is disposed of in full.
        let cost = amount("100");
        let qty = amount("3");
        assert_eq!(share(cost, qty, qty), Some(cost));
        let third = share(cost, amount("1"), qty).expect("a third of the cost");
        let two_thirds = share(cost, amount("2"), qty).expect("two thirds");
        // Each part is rounded at the 28th digit, so the two parts together
        // are a hair under the whole: that is why a disposal of everything
        // takes the whole cost, instead of adding the parts up.
        assert!(add(third, two_thirds).expect("a sum") <= cost);
        assert!(
            cost - add(third, two_thirds).expect("a sum") < amount("0.0000000000000000000000001")
        );
    }

    #[test]
    fn a_share_of_a_huge_total_falls_back_to_dividing_first() {
        let total = amount("10000000000000000000000000000");
        assert_eq!(
            share(total, amount("1"), amount("2")),
            Some(amount("5000000000000000000000000000"))
        );
    }

    #[test]
    fn stored_usd_is_cents_by_the_apps_own_rounding() {
        // `Amount::round` is banker's rounding: a midpoint goes to the even
        // cent. This test exists to pin which rounding the engine uses, so
        // nobody swaps in a second one.
        assert_eq!(usd(amount("0.005")), amount("0.00"));
        assert_eq!(usd(amount("0.015")), amount("0.02"));
        assert_eq!(usd(amount("-0.015")), amount("-0.02"));
        assert_eq!(usd(amount("0.004")), amount("0.00"));
        assert_eq!(usd(amount("0.006")), amount("0.01"));
        assert_eq!(usd(amount("1234.5678")), amount("1234.57"));
        // A quantity is not money: nothing here rounds one.
        assert_eq!(
            min(amount("0.000000000000000001"), amount("1")),
            amount("0.000000000000000001")
        );
    }
}
