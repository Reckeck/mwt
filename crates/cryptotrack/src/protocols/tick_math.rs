//! The concentrated-liquidity arithmetic both Uniswap adapters need (specs
//! D1, D2).
//!
//! A v3 position and a v4 position are the same geometry: liquidity spread
//! over a tick range, worth two token amounts at the pool's current price.
//! Spec D2 first copied these seventy lines out of `uniswap_v3.rs`, where
//! they were private; they live here instead, so that the one derivation is
//! checked once and a correction cannot land in one adapter only.
//!
//! Everything is integer arithmetic on [`U256`]: a tick ratio is a Q64.96
//! fixed-point word, exactly as `TickMath` returns it, and the constant table
//! of that library is deliberately *not* reproduced — the facts document
//! carries none, so the values are derived instead (see
//! [`sqrt_ratio_at_tick`]).

use super::wide::U256;
use mwt_base::error::{Error, Result};

fn overflow() -> Error {
    Error::Network("a tick computation does not fit a word".into())
}

/// The two tokens a liquidity amount is worth at `sqrt_price` (Q64.96), as
/// raw token units (`LiquidityAmounts.getAmountsForLiquidity`, the price
/// clamped into the range, so below it the position is all token0 and above
/// it all token1).
pub(super) fn principal(
    sqrt_price: U256,
    tick_lower: i64,
    tick_upper: i64,
    liquidity: u128,
) -> Result<(U256, U256)> {
    let lower = sqrt_ratio_at_tick(tick_lower)?;
    let upper = sqrt_ratio_at_tick(tick_upper)?;
    let price = sqrt_price.max(lower).min(upper);
    let q96 = U256::power_of_two(96).ok_or_else(overflow)?;
    let liquidity = U256::from_u128(liquidity);
    let one = U256::from_u128(1);
    // amount0 = L * 2^96 * (upper - price) / upper / price
    let amount0 = liquidity
        .checked_mul(q96)
        .and_then(|scaled| scaled.mul_div_floor(upper.wrapping_sub(price), upper))
        .and_then(|part| part.mul_div_floor(one, price))
        .ok_or_else(overflow)?;
    // amount1 = L * (price - lower) / 2^96
    let amount1 = liquidity
        .mul_div_floor(price.wrapping_sub(lower), q96)
        .ok_or_else(overflow)?;
    Ok((amount0, amount1))
}

/// `sqrt(1.0001^tick)` as Q64.96, the value `TickMath.getSqrtRatioAtTick`
/// returns — computed in big integers, not from that library's table of
/// constants (the facts document carries none).
///
/// The base `sqrt(1.0001)` is taken to 127 fractional bits by an integer
/// square root and raised by squaring in Q127; each step floors, so the
/// relative error stays below `2^-100` for any tick, far beneath the `2^-32`
/// the Q96 result itself resolves at the range's ends. A negative tick is the
/// reciprocal.
pub(super) fn sqrt_ratio_at_tick(tick: i64) -> Result<U256> {
    let q127 = U256::power_of_two(127).ok_or_else(overflow)?;
    let q254 = U256::power_of_two(254).ok_or_else(overflow)?;
    let one = U256::from_u128(1);
    // 1.0001 in Q254, then its root in Q127.
    let ratio = U256::from_u128(10_001)
        .mul_div_floor(q254, U256::from_u128(10_000))
        .ok_or_else(overflow)?;
    let mut base = integer_sqrt(ratio)?;
    let mut result = q127;
    let mut exponent = tick.unsigned_abs();
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = result.mul_div_floor(base, q127).ok_or_else(overflow)?;
        }
        exponent >>= 1;
        if exponent > 0 {
            base = base.mul_div_floor(base, q127).ok_or_else(overflow)?;
        }
    }
    if tick < 0 {
        result = q127.mul_div_floor(q127, result).ok_or_else(overflow)?;
    }
    // Q127 -> Q96.
    let shift = U256::power_of_two(31).ok_or_else(overflow)?;
    result.mul_div_floor(one, shift).ok_or_else(overflow)
}

/// `floor(sqrt(value))` by Newton's iteration from above.
pub(super) fn integer_sqrt(value: U256) -> Result<U256> {
    if value.is_zero() {
        return Ok(U256::ZERO);
    }
    let one = U256::from_u128(1);
    let two = U256::from_u128(2);
    // 2^128 is at least the root of any word.
    let mut guess = U256::power_of_two(128).ok_or_else(overflow)?;
    loop {
        let next = guess
            .checked_add(value.mul_div_floor(one, guess).ok_or_else(overflow)?)
            .and_then(|sum| sum.mul_div_floor(one, two))
            .ok_or_else(overflow)?;
        if next >= guess {
            return Ok(guess);
        }
        guess = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(decimal: &str) -> U256 {
        U256::from_decimal(decimal).expect("a decimal")
    }

    /// `sqrt(1.0001^tick) * 2^96` computed outside Rust to 120 digits.
    #[test]
    fn the_sqrt_ratio_of_a_tick_matches_an_independent_computation() {
        let expected = [
            (0, "79228162514264337593543950336"),
            (1, "79232123823359799118286999567"),
            (-1, "79224201403219477170569942573"),
            (60, "79466191966197645195421774832"),
            (-200_500, "3509902707678499862373137"),
            (887_272, "1461446703485210103244672773810124308346321380902"),
            (-887_272, "4295128738"),
        ];
        for (tick, text) in expected {
            let got = sqrt_ratio_at_tick(tick).expect("a ratio");
            let want = raw(text);
            let difference = if got >= want {
                got.wrapping_sub(want)
            } else {
                want.wrapping_sub(got)
            };
            // One unit at the bottom of the range, 1e-15 relative elsewhere.
            let relative = want
                .mul_div_floor(raw("1"), raw("1000000000000000"))
                .expect("a bound");
            let allowed = relative.max(raw("2"));
            assert!(difference <= allowed, "tick {tick}: {got} against {want}");
        }
        // The protocol's own published bounds for the two ends of the range.
        // `MIN_SQRT_RATIO` is published as 4295128739 and this derivation
        // gives 4295128738: one unit of a Q96 word at the very bottom of the
        // range, which is why the bounds below are a unit either side rather
        // than an equality.
        let floor = sqrt_ratio_at_tick(-887_272).expect("min");
        assert!(floor >= raw("4295128737"));
        assert!(floor <= raw("4295128740"));
    }

    #[test]
    fn the_amounts_of_a_range_are_all_token0_below_it_and_all_token1_above_it() {
        let lower = sqrt_ratio_at_tick(-1_000).expect("lower");
        let upper = sqrt_ratio_at_tick(1_000).expect("upper");
        let below = principal(lower.wrapping_sub(raw("1")), -1_000, 1_000, 1 << 60).expect("below");
        assert!(!below.0.is_zero());
        assert!(below.1.is_zero(), "below the range there is no token1");
        let above = principal(
            upper.checked_add(raw("1")).expect("sum"),
            -1_000,
            1_000,
            1 << 60,
        )
        .expect("above");
        assert!(above.0.is_zero(), "above the range there is no token0");
        assert!(!above.1.is_zero());
        let none = principal(lower, -1_000, 1_000, 0).expect("no liquidity");
        assert_eq!(none, (U256::ZERO, U256::ZERO));
    }

    #[test]
    fn the_integer_root_floors_and_never_loops_forever() {
        assert_eq!(integer_sqrt(U256::ZERO).expect("zero"), U256::ZERO);
        assert_eq!(
            integer_sqrt(U256::from_u128(1)).expect("one"),
            U256::from_u128(1)
        );
        // 2^254 has an exact root; 2^255 does not, and the floor is taken.
        let even = U256::power_of_two(254).expect("fits");
        assert_eq!(
            integer_sqrt(even).expect("root"),
            U256::power_of_two(127).expect("fits")
        );
        let odd = U256::power_of_two(255).expect("fits");
        let root = integer_sqrt(odd).expect("root");
        assert!(root.checked_mul(root).expect("square") <= odd);
        let next = root.checked_add(U256::from_u128(1)).expect("next");
        assert!(
            next.checked_mul(next).is_none_or(|square| square > odd),
            "the floor is the largest root whose square still fits under the value"
        );
        // 10^10 squared is 10^20, so the root of 10^20 + 1 is 10^10.
        assert_eq!(
            integer_sqrt(raw("100000000000000000001")).expect("root"),
            raw("10000000000")
        );
    }
}
