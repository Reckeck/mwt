//! Unsigned integers as wide as a chain word (spec D0, addendum for wave 1).
//!
//! A protocol answers in 256-bit words, and the arithmetic wave 1 needs does
//! not fit anything the application already has:
//!
//! * [`mwt_base::Amount`] is a `rust_decimal::Decimal`: 96 bits of mantissa, so
//!   it cannot even hold `2^128`, let alone a fee growth accumulator;
//! * [`crate::sources::evm::Uint256`] is a *checked decimal
//!   string* — it reads a word and scales it into an `Amount`, and that is
//!   all it does. There is no arithmetic on it on purpose.
//!
//! What the adapters have to compute:
//!
//! * Uniswap v3/v4: `tokensOwed + liquidity * (feeGrowthInside_now -
//!   feeGrowthInside_then) / 2^128`, where the subtraction wraps modulo
//!   `2^256` by the protocol's own design (spec D1, D2) — the intermediate
//!   product is up to 384 bits wide;
//! * Aave v3: a balance times a liquidity index in ray (`1e27`) divided by
//!   ray again (spec D3);
//! * Lido: a wstETH balance times `stEthPerToken` divided by `1e18`
//!   (spec D4).
//!
//! All of them are one shape: **multiply two words, then divide by a third,
//! and never lose a digit in between** ([`U256::mul_div_floor`], exact down
//! to the last unit and `None` only when the result would not fit a word or
//! the divisor is zero). `f64` is out of the question (`AGENTS.md`, rule 2),
//! and so is rounding in the middle.
//!
//! # Why not a crate
//!
//! A wallet's dependency list is part of its attack surface, and the whole
//! of what is needed here is a multiply, a divide and a wrapping subtract on
//! four limbs — some two hundred lines with the tests that pin them. The
//! decision and what would change it are written down in the ADR
//! `trade-app.crypto.adr-wide-integers`.
//!
//! The limbs are little-endian (`limbs[0]` is the least significant), the
//! type is `Copy`, and every operation is total: nothing here panics and
//! nothing wraps unless its name says so. The order is numeric: `Ord` is
//! written by hand, because a derived one would compare the least
//! significant limb first.

use core::cmp::Ordering;

use crate::sources::evm::Uint256;
use mwt_base::Amount;
use mwt_base::error::{Error, Result};

/// Number of 64-bit limbs in a chain word.
const LIMBS: usize = 4;

/// Number of limbs in the intermediate of a multiplication: two words wide,
/// so a product is never truncated before the division.
const WIDE_LIMBS: usize = LIMBS * 2;

/// An unsigned integer of exactly 256 bits, as a chain word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct U256 {
    /// Little-endian: `limbs[0]` holds bits 0..64.
    limbs: [u64; LIMBS],
}

/// Numeric order, most significant limb first.
///
/// This cannot be derived: the limbs are little-endian, and a derived `Ord`
/// would compare `limbs[0]` first and call `2^64` smaller than `5`.
impl Ord for U256 {
    fn cmp(&self, other: &Self) -> Ordering {
        for (mine, theirs) in self.limbs.iter().rev().zip(other.limbs.iter().rev()) {
            match mine.cmp(theirs) {
                Ordering::Equal => {}
                verdict => return verdict,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl U256 {
    pub const ZERO: Self = Self { limbs: [0; LIMBS] };

    /// `2^256 - 1`: what a wrapping subtraction of one from zero answers.
    pub const MAX: Self = Self {
        limbs: [u64::MAX; LIMBS],
    };

    pub const fn from_u128(value: u128) -> Self {
        Self {
            limbs: [value as u64, (value >> 64) as u64, 0, 0],
        }
    }

    /// `2^exponent`, and `None` for an exponent of 256 or more — the one
    /// place a caller asks for a power is `2^128` (Uniswap's Q128.128) and
    /// `1e27` is not a power of two, so this stays small on purpose.
    pub fn power_of_two(exponent: u32) -> Option<Self> {
        if exponent >= 256 {
            return None;
        }
        let mut limbs = [0u64; LIMBS];
        limbs[(exponent / 64) as usize] = 1u64 << (exponent % 64);
        Some(Self { limbs })
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.iter().all(|limb| *limb == 0)
    }

    /// One 32-byte word as a node returns it: `0x` and up to 64 hex digits,
    /// shorter when the node trims leading zeros. Longer than a word, or a
    /// digit that is not hex, is an error and never a silent truncation.
    pub fn from_hex(text: &str) -> Result<Self> {
        let digits = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"));
        let digits = digits.ok_or_else(|| Self::bad(text, "a word starts with 0x"))?;
        let digits = digits.trim_start_matches('0');
        if digits.len() > 64 {
            return Err(Self::bad(text, "a word is 32 bytes at most"));
        }
        let mut limbs = [0u64; LIMBS];
        for (index, digit) in digits.chars().rev().enumerate() {
            let value = digit
                .to_digit(16)
                .ok_or_else(|| Self::bad(text, "not a hex digit"))? as u64;
            limbs[index / 16] |= value << ((index % 16) * 4);
        }
        Ok(Self { limbs })
    }

    /// A decimal written out in full, as a fixture or a test states it.
    pub fn from_decimal(text: &str) -> Result<Self> {
        let digits = text.trim();
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Self::bad(text, "a decimal is digits only"));
        }
        let mut value = Self::ZERO;
        for digit in digits.bytes() {
            value = value
                .checked_mul_small(10)
                .and_then(|scaled| scaled.checked_add_small(u64::from(digit - b'0')))
                .ok_or_else(|| Self::bad(text, "larger than a word"))?;
        }
        Ok(value)
    }

    /// The value written out in decimal: what [`Uint256`] and so every
    /// existing conversion into an [`Amount`] speaks.
    pub fn to_decimal(self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let mut digits = Vec::new();
        let mut rest = self;
        while !rest.is_zero() {
            let (quotient, remainder) = rest.divmod_small(10);
            digits.push(b'0' + remainder as u8);
            rest = quotient;
        }
        digits.reverse();
        String::from_utf8(digits).expect("decimal digits are ascii")
    }

    /// The value as whole units of a token with `decimals` places, through
    /// the one conversion the module already has ([`Uint256::scaled`]): an
    /// exact [`Amount`] or an error, never a rounded one.
    pub fn to_amount(self, decimals: u8) -> Result<Amount> {
        Uint256::from_decimal(&self.to_decimal())?.scaled(decimals)
    }

    /// The value as one 32-byte big-endian word: an argument of a call, the
    /// way the node wants it.
    pub fn to_be_bytes(self) -> [u8; 32] {
        let mut word = [0u8; 32];
        for (index, limb) in self.limbs.iter().enumerate() {
            let at = 32 - (index + 1) * 8;
            word[at..at + 8].copy_from_slice(&limb.to_be_bytes());
        }
        word
    }

    /// One 32-byte word of a node's answer.
    pub fn from_be_bytes(word: &[u8; 32]) -> Self {
        let mut limbs = [0u64; LIMBS];
        for (index, limb) in limbs.iter_mut().enumerate() {
            let at = 32 - (index + 1) * 8;
            let mut eight = [0u8; 8];
            eight.copy_from_slice(&word[at..at + 8]);
            *limb = u64::from_be_bytes(eight);
        }
        Self { limbs }
    }

    /// `self - other` modulo `2^256`.
    ///
    /// This is not a convenience: a Uniswap fee growth accumulator is
    /// *defined* to wrap, so the difference between two readings is correct
    /// only when the subtraction wraps the same way (spec D1).
    pub fn wrapping_sub(self, other: Self) -> Self {
        let mut limbs = [0u64; LIMBS];
        let mut borrow = 0u64;
        let pairs = self.limbs.iter().zip(other.limbs.iter());
        for (limb, (left, right)) in limbs.iter_mut().zip(pairs) {
            let (difference, first_borrow) = left.overflowing_sub(*right);
            let (difference, second_borrow) = difference.overflowing_sub(borrow);
            *limb = difference;
            borrow = u64::from(first_borrow) + u64::from(second_borrow);
        }
        Self { limbs }
    }

    /// `self + other`, and `None` when the sum would not fit a word: a fee
    /// that overflowed a word is a bug to report, not a number to wrap.
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let mut limbs = [0u64; LIMBS];
        let mut carry = 0u64;
        let pairs = self.limbs.iter().zip(other.limbs.iter());
        for (limb, (left, right)) in limbs.iter_mut().zip(pairs) {
            let (sum, first_carry) = left.overflowing_add(*right);
            let (sum, second_carry) = sum.overflowing_add(carry);
            *limb = sum;
            carry = u64::from(first_carry) + u64::from(second_carry);
        }
        (carry == 0).then_some(Self { limbs })
    }

    /// `self * factor / divisor`, floored, computed on the full 512-bit
    /// product so nothing is lost in the middle.
    ///
    /// `None` when `divisor` is zero or when the result does not fit a word
    /// — both of which mean the reading is wrong, and an adapter answers
    /// with an absent figure rather than a guess.
    pub fn mul_div_floor(self, factor: Self, divisor: Self) -> Option<Self> {
        if divisor.is_zero() {
            return None;
        }
        let product = widening_mul(&self.limbs, &factor.limbs);
        let mut wide_divisor = [0u64; WIDE_LIMBS];
        wide_divisor[..LIMBS].copy_from_slice(&divisor.limbs);
        let (quotient, _) = divmod_wide(product, wide_divisor);
        narrow(quotient)
    }

    /// `self * factor`, and `None` when the product does not fit a word.
    pub fn checked_mul(self, factor: Self) -> Option<Self> {
        narrow(widening_mul(&self.limbs, &factor.limbs))
    }

    fn checked_mul_small(self, factor: u64) -> Option<Self> {
        let mut limbs = [0u64; LIMBS];
        let mut carry = 0u128;
        for (limb, source) in limbs.iter_mut().zip(self.limbs.iter()) {
            let product = u128::from(*source) * u128::from(factor) + carry;
            *limb = product as u64;
            carry = product >> 64;
        }
        (carry == 0).then_some(Self { limbs })
    }

    fn checked_add_small(self, addend: u64) -> Option<Self> {
        self.checked_add(Self {
            limbs: [addend, 0, 0, 0],
        })
    }

    fn divmod_small(self, divisor: u64) -> (Self, u64) {
        let mut limbs = [0u64; LIMBS];
        let mut remainder = 0u128;
        for index in (0..LIMBS).rev() {
            let current = (remainder << 64) | u128::from(self.limbs[index]);
            limbs[index] = (current / u128::from(divisor)) as u64;
            remainder = current % u128::from(divisor);
        }
        (Self { limbs }, remainder as u64)
    }

    fn bad(text: &str, why: &str) -> Error {
        Error::InvalidInput(format!("crypto: {why}: {}", shorten(text)))
    }
}

impl std::fmt::Display for U256 {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.to_decimal())
    }
}

/// A word of a fixture or a log, cut down for an error message.
fn shorten(text: &str) -> String {
    if text.chars().count() <= 24 {
        return text.to_string();
    }
    let head: String = text.chars().take(21).collect();
    format!("{head}…")
}

/// The full product of two words: schoolbook, into twice the limbs, so no
/// digit is dropped before the division that follows.
fn widening_mul(left: &[u64; LIMBS], right: &[u64; LIMBS]) -> [u64; WIDE_LIMBS] {
    let mut product = [0u64; WIDE_LIMBS];
    for (left_index, left_limb) in left.iter().enumerate() {
        if *left_limb == 0 {
            continue;
        }
        let mut carry = 0u128;
        for (right_index, right_limb) in right.iter().enumerate() {
            let position = left_index + right_index;
            let current = u128::from(product[position])
                + u128::from(*left_limb) * u128::from(*right_limb)
                + carry;
            product[position] = current as u64;
            carry = current >> 64;
        }
        let mut position = left_index + LIMBS;
        while carry != 0 && position < WIDE_LIMBS {
            let current = u128::from(product[position]) + carry;
            product[position] = current as u64;
            carry = current >> 64;
            position += 1;
        }
    }
    product
}

/// `numerator / divisor` and its remainder, both 512 bits wide.
///
/// Bitwise long division: 512 shifts and subtractions. Chosen for being
/// plainly correct rather than fast — a pass reads a handful of positions,
/// and the whole of the arithmetic a reviewer has to trust is in these
/// twenty lines.
fn divmod_wide(
    numerator: [u64; WIDE_LIMBS],
    divisor: [u64; WIDE_LIMBS],
) -> ([u64; WIDE_LIMBS], [u64; WIDE_LIMBS]) {
    let mut quotient = [0u64; WIDE_LIMBS];
    let mut remainder = [0u64; WIDE_LIMBS];
    for bit in (0..WIDE_LIMBS * 64).rev() {
        shift_left_one(&mut remainder);
        if bit_of(&numerator, bit) {
            remainder[0] |= 1;
        }
        if compare_wide(&remainder, &divisor) != std::cmp::Ordering::Less {
            subtract_wide(&mut remainder, &divisor);
            quotient[bit / 64] |= 1u64 << (bit % 64);
        }
    }
    (quotient, remainder)
}

fn bit_of(value: &[u64; WIDE_LIMBS], bit: usize) -> bool {
    value[bit / 64] >> (bit % 64) & 1 == 1
}

fn shift_left_one(value: &mut [u64; WIDE_LIMBS]) {
    let mut carry = 0u64;
    for limb in value.iter_mut() {
        let next_carry = *limb >> 63;
        *limb = (*limb << 1) | carry;
        carry = next_carry;
    }
}

fn compare_wide(left: &[u64; WIDE_LIMBS], right: &[u64; WIDE_LIMBS]) -> std::cmp::Ordering {
    for index in (0..WIDE_LIMBS).rev() {
        match left[index].cmp(&right[index]) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

fn subtract_wide(left: &mut [u64; WIDE_LIMBS], right: &[u64; WIDE_LIMBS]) {
    let mut borrow = 0u64;
    for index in 0..WIDE_LIMBS {
        let (difference, first_borrow) = left[index].overflowing_sub(right[index]);
        let (difference, second_borrow) = difference.overflowing_sub(borrow);
        left[index] = difference;
        borrow = u64::from(first_borrow) + u64::from(second_borrow);
    }
}

/// A 512-bit value back into a word, or `None` when it does not fit.
fn narrow(wide: [u64; WIDE_LIMBS]) -> Option<U256> {
    if wide[LIMBS..].iter().any(|limb| *limb != 0) {
        return None;
    }
    let mut limbs = [0u64; LIMBS];
    limbs.copy_from_slice(&wide[..LIMBS]);
    Some(U256 { limbs })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `2^128`, the divisor of every Uniswap fee growth figure.
    fn q128() -> U256 {
        U256::power_of_two(128).expect("128 is under 256")
    }

    #[test]
    fn a_word_reads_from_hex_the_way_a_node_writes_it() {
        assert_eq!(U256::from_hex("0x0").expect("zero"), U256::ZERO);
        assert_eq!(
            U256::from_hex("0x1f").expect("a byte").to_decimal(),
            "31",
            "a node trims the leading zeros"
        );
        let padded = format!("0x{:064x}", 31u8);
        assert_eq!(
            U256::from_hex(&padded).expect("a padded word").to_decimal(),
            "31"
        );
        assert_eq!(
            U256::from_hex(&format!("0x{}", "f".repeat(64))).expect("max"),
            U256::MAX
        );
    }

    #[test]
    fn a_word_that_is_not_one_is_refused_and_not_cut_short() {
        assert!(U256::from_hex("1f").is_err(), "no 0x");
        assert!(U256::from_hex("0xzz").is_err(), "not hex");
        assert!(
            U256::from_hex(&format!("0x1{}", "0".repeat(64))).is_err(),
            "2^256 is one digit too wide"
        );
        assert!(U256::from_decimal("").is_err());
        assert!(U256::from_decimal("12a").is_err());
    }

    #[test]
    fn the_largest_word_round_trips_through_decimal() {
        let max = U256::MAX.to_decimal();
        assert_eq!(
            max,
            "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        );
        assert_eq!(
            U256::from_decimal(&max).expect("the largest word"),
            U256::MAX
        );
        assert!(
            U256::from_decimal(
                "115792089237316195423570985008687907853269984665640564039457584007913129639936"
            )
            .is_err(),
            "2^256 does not fit a word"
        );
    }

    #[test]
    fn a_word_is_the_same_value_in_bytes_and_in_hex() {
        for text in [
            "0",
            "1",
            "340282366920938463463374607431768211455",
            "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        ] {
            let value = U256::from_decimal(text).expect("a word");
            assert_eq!(U256::from_be_bytes(&value.to_be_bytes()), value);
        }
        let word = U256::from_u128(0x0102_0304).to_be_bytes();
        assert_eq!(word[28..], [1, 2, 3, 4], "big-endian, the way a node reads");
        assert!(word[..28].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn a_fee_growth_difference_wraps_the_way_the_protocol_defines_it() {
        // The accumulator is allowed to pass 2^256 and start again, so the
        // difference between a later and an earlier reading is taken modulo
        // 2^256 (spec D1). Without the wrap this reads as an enormous fee.
        let then = U256::MAX;
        let now = U256::from_u128(4);
        assert_eq!(now.wrapping_sub(then).to_decimal(), "5");
        assert_eq!(U256::ZERO.wrapping_sub(U256::from_u128(1)), U256::MAX);
        assert_eq!(
            U256::from_u128(7).wrapping_sub(U256::from_u128(7)),
            U256::ZERO
        );
    }

    #[test]
    fn a_fee_is_liquidity_times_the_growth_divided_by_two_to_the_128() {
        // Worked by hand: liquidity 10^18, growth difference 2^128 / 1000,
        // so the fee is 10^18 / 1000 = 10^15 units.
        let liquidity = U256::from_decimal("1000000000000000000").expect("liquidity");
        let growth = q128()
            .mul_div_floor(U256::from_u128(1), U256::from_u128(1000))
            .expect("a growth of a thousandth");
        assert_eq!(
            liquidity
                .mul_div_floor(growth, q128())
                .expect("a fee")
                .to_decimal(),
            "999999999999999",
            "floored: the growth itself was floored first"
        );
    }

    #[test]
    fn the_product_is_not_truncated_before_the_division() {
        // (2^255) * 4 / 8 = 2^254: the product overflows a word and the
        // result does not. A 256-bit intermediate would answer zero here.
        let half = U256::power_of_two(255).expect("2^255");
        assert_eq!(
            half.mul_div_floor(U256::from_u128(4), U256::from_u128(8))
                .expect("it fits again"),
            U256::power_of_two(254).expect("2^254")
        );
        assert_eq!(
            U256::MAX
                .mul_div_floor(U256::MAX, U256::MAX)
                .expect("the largest word back"),
            U256::MAX
        );
    }

    #[test]
    fn a_division_that_cannot_answer_says_so_instead_of_guessing() {
        assert!(
            U256::from_u128(1)
                .mul_div_floor(U256::from_u128(1), U256::ZERO)
                .is_none(),
            "a divisor of zero"
        );
        assert!(
            U256::MAX
                .mul_div_floor(U256::from_u128(2), U256::from_u128(1))
                .is_none(),
            "twice the largest word does not fit one"
        );
        assert!(U256::MAX.checked_mul(U256::from_u128(2)).is_none());
        assert!(U256::MAX.checked_add(U256::from_u128(1)).is_none());
        assert!(U256::power_of_two(256).is_none());
    }

    #[test]
    fn the_order_is_numeric_and_not_the_order_of_the_limbs() {
        // The limbs are little-endian, so a derived `Ord` compares bits 0..64
        // first: it would call 2^64 smaller than 5. Every comparison in an
        // adapter (a square root, a clamp to a range) depends on this.
        let small = U256::from_u128(5);
        let big = U256::power_of_two(64).expect("2^64 fits a word");
        assert!(small < big, "5 is less than 2^64");
        assert!(big > small);
        assert_eq!(small.max(big), big);
        assert_eq!(small.min(big), small);
        assert!(U256::ZERO < U256::from_u128(1));
        assert!(U256::MAX > U256::power_of_two(255).expect("2^255 fits a word"));
        let mut ladder = vec![
            U256::MAX,
            U256::from_u128(5),
            U256::power_of_two(64).expect("fits"),
            U256::ZERO,
            U256::power_of_two(192).expect("fits"),
        ];
        ladder.sort();
        assert_eq!(
            ladder,
            vec![
                U256::ZERO,
                U256::from_u128(5),
                U256::power_of_two(64).expect("fits"),
                U256::power_of_two(192).expect("fits"),
                U256::MAX,
            ]
        );
        // Equal words compare equal through every limb.
        assert_eq!(
            U256::from_hex(&format!("0x{:064x}", 1)).expect("one"),
            U256::from_u128(1)
        );
    }

    #[test]
    fn a_ray_index_is_applied_without_losing_a_unit() {
        // Aave keeps its liquidity index in ray (1e27): a balance of
        // 1234567890123456789 scaled by an index of 1.5 ray is exactly
        // 1851851835185185183.5, floored to .3 (spec D3).
        let ray = U256::from_decimal("1000000000000000000000000000").expect("ray");
        let index = U256::from_decimal("1500000000000000000000000000").expect("1.5 ray");
        let balance = U256::from_decimal("1234567890123456789").expect("a balance");
        assert_eq!(
            balance
                .mul_div_floor(index, ray)
                .expect("the scaled balance")
                .to_decimal(),
            "1851851835185185183"
        );
    }

    #[test]
    fn a_word_becomes_whole_units_or_an_error_and_never_a_rounded_number() {
        let wei = U256::from_decimal("1500000000000000000").expect("1.5 ether in wei");
        assert_eq!(wei.to_amount(18).expect("whole units").to_string(), "1.5");
        assert!(
            U256::MAX.to_amount(18).is_err(),
            "the largest word is wider than an Amount, and that is an error, not a rounding"
        );
    }

    /// Vectors computed by a second, independent implementation of the same
    /// arithmetic (arbitrary-precision integers, outside this crate), pinned
    /// here: random words, random divisors, including two whose quotient does
    /// not fit a word. If a limb of [`widening_mul`] or a shift of
    /// [`divmod_wide`] is ever wrong, one of these stops matching.
    #[test]
    fn the_wide_arithmetic_matches_an_independent_implementation() {
        let vectors: [(&str, &str, &str, Option<&str>); 8] = [
            (
                "108728716244441019309549147683173588123733819230613795803148713172350594057668",
                "74454207780974370328756138403899997625654298313327020283828571632431783557034",
                "67070589467114295589245390063906932391340814033608919242342744054643213145812",
                None,
            ),
            (
                "18268332553777465850549931961174900117369788587186360746846284508608190947343",
                "89566000925040707295756325302123285898662381944827046931639213429147896461506",
                "57553055060419299769944211684152657884958596557182156485386301746460655254108",
                Some(
                    "28429793843139613291151721477104162018708903050970859139572824221025063956138",
                ),
            ),
            (
                "44629454741855486634272263559652759900995808919065922557301314591886471057558",
                "3579167639574933824203807326110810155561016567030034992722133927151561113658",
                "108756910961959950133514422499361930717330069806340031695598828182439695216179",
                Some(
                    "1468746204457704931704363586344526356950260813855788844636035987385157840221",
                ),
            ),
            (
                "76210938249744152244373885811902994101124721888924295449273744434075091963474",
                "111127172295630458258428565451186390404873498459261563646568539596288016782189",
                "110883639054610128798460015097585957717202039546020136679170559993595219847186",
                Some(
                    "76378319992906636742944225848123929585253171169162833957853779391805216131256",
                ),
            ),
            (
                "73635172312142733450634384841581479452219733575575560116513469071763089862199",
                "77311839885015963803114246856128972757180071101694223397591961326757745827165",
                "107946686995257731367501816684395434983781638130821859460070997622047116730004",
                Some(
                    "52737798724216858603836633905571865245077952357636532829610170098551876064937",
                ),
            ),
            (
                "60434216827632395292213465850624361933232988078529122699588943998784958643459",
                "104797418751902193189943199424894296869286534087134085498560079167991356914753",
                "109422715562919327970097193020875438601172936299859479989026582356127342457991",
                Some(
                    "57879663242198524508908099948124904236054382133448608393578618465452391243442",
                ),
            ),
            (
                "86247095717870456741897351370587567292827811558009133054452689213385657769288",
                "43665416072951153884175630636037372806330219539777288935537603487221109259702",
                "77185429701149392325764449272947107604255594022990840691500461819989955918576",
                Some(
                    "48791790551479904113332517726920606985986647159432884823009922037062834645167",
                ),
            ),
            (
                "83876792320495957376746506381197153361019315417699891919242164564124736462841",
                "110456818114379853125033624885542337997042069318489147745449796859699893712970",
                "32878579274036713337512082819682544427785991893557053320399075544311619818231",
                None,
            ),
        ];
        for (left_text, factor_text, divisor_text, expected) in vectors {
            let left = U256::from_decimal(left_text).expect("a word");
            let factor = U256::from_decimal(factor_text).expect("a word");
            let divisor = U256::from_decimal(divisor_text).expect("a word");
            let answered = left.mul_div_floor(factor, divisor).map(U256::to_decimal);
            assert_eq!(
                answered.as_deref(),
                expected,
                "left {left_text}, factor {factor_text}, divisor {divisor_text}"
            );
        }

        let wraps: [(&str, &str, &str); 4] = [
            (
                "39700599040030819961839706339663813870486589235036806359485654157150460342623",
                "87378641560089223446647063693300087182630810091523640848500153328115950597672",
                "68114046717257791938763627655051634541125763809153729550443084836947639384887",
            ),
            (
                "40286818565656694706990416813959784124805395348332868011264104877536620567277",
                "325081006931880959931396412062581703127804831988867338695333582357925316463",
                "39961737558724813747059020401897202421677590516344000672568771295178695250814",
            ),
            (
                "10760170410403651137435087750191606304851827485714844365316223423173332778954",
                "70523051463365632024849386709353811636920932407049018947884924807417296975197",
                "56029208184354214536156686049525702521200879744306389456888882623669165443693",
            ),
            (
                "113664150757380986108400806508685010106873810747949489874646648975743352243159",
                "43838852408142714645551277463237135317618375477647435934809553765621166041090",
                "69825298349238271462849529045447874789255435270302053939837095210122186202069",
            ),
        ];
        for (left_text, right_text, expected) in wraps {
            let left = U256::from_decimal(left_text).expect("a word");
            let right = U256::from_decimal(right_text).expect("a word");
            assert_eq!(
                left.wrapping_sub(right).to_decimal(),
                expected,
                "{left_text} - {right_text} modulo 2^256"
            );
        }
    }

    /// Cross-check against the same arithmetic done limb by limb in a
    /// different order: a product divided by one of its own factors is the
    /// other factor exactly.
    #[test]
    fn multiplication_and_division_are_each_others_inverse_on_known_pairs() {
        let pairs = [
            (
                "340282366920938463463374607431768211456",
                "999999999999999999",
            ),
            ("18446744073709551616", "18446744073709551615"),
            (
                "1",
                "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            ),
            ("1000000000000000000000000000", "1000000000000000000"),
        ];
        for (left_text, right_text) in pairs {
            let left = U256::from_decimal(left_text).expect("a left factor");
            let right = U256::from_decimal(right_text).expect("a right factor");
            let product = left.checked_mul(right).expect("the product fits a word");
            assert_eq!(product.mul_div_floor(U256::from_u128(1), right), Some(left));
            assert_eq!(product.mul_div_floor(U256::from_u128(1), left), Some(right));
        }
    }
}
