//! `uint256` without a float and without a big-integer crate.
//!
//! A token quantity from the network is an integer up to 2^256 - 1 (78
//! digits). `Amount` (a 96-bit decimal) holds at most 28-29 digits, so the
//! raw integer travels as [`Uint256`] (its decimal digits) and is turned into
//! an `Amount` only when it fits exactly. A value that does not fit is an
//! error, never a rounded number: junk tokens routinely report 10^70 units.

use serde::{Deserialize, Serialize};

use mwt_base::Amount;
use mwt_base::error::{Error, Result};

/// 2^256 - 1, the largest `uint256`.
const MAX_DECIMAL: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935";

/// A non-negative integer below 2^256, kept as canonical decimal digits (no
/// leading zeros, `"0"` for zero), so nothing is ever rounded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Uint256(String);

impl Uint256 {
    pub fn zero() -> Self {
        Self("0".into())
    }

    pub fn is_zero(&self) -> bool {
        self.0 == "0"
    }

    /// The canonical decimal digits.
    pub fn as_decimal(&self) -> &str {
        &self.0
    }

    /// From decimal digits as an explorer prints them.
    pub fn from_decimal(text: &str) -> Result<Self> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::InvalidInput(format!(
                "not an unsigned integer: {}",
                shorten(text)
            )));
        }
        let digits = text.trim_start_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        let too_big = digits.len() > MAX_DECIMAL.len()
            || (digits.len() == MAX_DECIMAL.len() && digits > MAX_DECIMAL);
        if too_big {
            return Err(Error::InvalidInput("integer above 2^256 - 1".into()));
        }
        Ok(Self(digits.to_owned()))
    }

    /// From a `0x` hex quantity (a JSON-RPC result or one ABI word).
    pub fn from_hex(text: &str) -> Result<Self> {
        let nibbles = text
            .strip_prefix("0x")
            .or_else(|| text.strip_prefix("0X"))
            .ok_or_else(|| Error::InvalidInput(format!("not a hex quantity: {}", shorten(text))))?;
        if nibbles.is_empty() || nibbles.len() > 64 {
            return Err(Error::InvalidInput(
                "hex quantity of 1..=64 digits expected".into(),
            ));
        }
        // Base-10 digits, least significant first: digits = digits * 16 + nibble.
        let mut digits: Vec<u8> = vec![0];
        for ch in nibbles.chars() {
            let nibble = ch.to_digit(16).ok_or_else(|| {
                Error::InvalidInput(format!("not a hex quantity: {}", shorten(text)))
            })?;
            let mut carry = nibble;
            for digit in &mut digits {
                let cell = u32::from(*digit) * 16 + carry;
                *digit = (cell % 10) as u8;
                carry = cell / 10;
            }
            while carry > 0 {
                digits.push((carry % 10) as u8);
                carry /= 10;
            }
        }
        let text: String = digits
            .iter()
            .rev()
            .map(|digit| char::from(b'0' + digit))
            .collect();
        Self::from_decimal(&text)
    }

    /// Whole units for a token of `decimals` places, exactly or not at all.
    pub fn scaled(&self, decimals: u8) -> Result<Amount> {
        let places = usize::from(decimals);
        let text = if places == 0 {
            self.0.clone()
        } else {
            let padded = format!("{:0>width$}", self.0, width = places + 1);
            let (whole, fraction) = padded.split_at(padded.len() - places);
            let fraction = fraction.trim_end_matches('0');
            if fraction.is_empty() {
                whole.to_owned()
            } else {
                format!("{whole}.{fraction}")
            }
        };
        // `from_str_exact` refuses anything that would be rounded.
        rust_decimal::Decimal::from_str_exact(&text)
            .map(Amount::new)
            .map_err(|_| {
                Error::InvalidInput(format!(
                    "{} with {decimals} decimals does not fit an exact amount",
                    self.0
                ))
            })
    }
}

impl std::fmt::Display for Uint256 {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}

/// A bounded excerpt of untrusted text for an error message.
pub(super) fn shorten(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control())
        .take(40)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_round_trips_the_largest_value_without_loss() {
        let max = Uint256::from_decimal(MAX_DECIMAL).unwrap();
        assert_eq!(max.as_decimal(), MAX_DECIMAL);
        assert!(Uint256::from_decimal(&format!("{MAX_DECIMAL}0")).is_err());
        let over = "115792089237316195423570985008687907853269984665640564039457584007913129639936";
        assert!(Uint256::from_decimal(over).is_err(), "2^256 is out");
    }

    #[test]
    fn hex_of_all_ones_is_2_pow_256_minus_1() {
        let hex = format!("0x{}", "f".repeat(64));
        assert_eq!(Uint256::from_hex(&hex).unwrap().as_decimal(), MAX_DECIMAL);
        assert!(Uint256::from_hex(&format!("0x{}", "f".repeat(65))).is_err());
    }

    #[test]
    fn hex_matches_known_values() {
        assert_eq!(Uint256::from_hex("0x0").unwrap().as_decimal(), "0");
        assert_eq!(Uint256::from_hex("0x00ff").unwrap().as_decimal(), "255");
        // eth_getBalance answer captured from a real node (fixtures/rpc.getBalance.json).
        assert_eq!(
            Uint256::from_hex("0x102d4377865f1746c07")
                .unwrap()
                .as_decimal(),
            "4774551794456062094343"
        );
    }

    #[test]
    fn junk_is_refused() {
        for bad in ["", "-1", "1.5", "1e18", "0x10", " 1", "١٢"] {
            assert!(Uint256::from_decimal(bad).is_err(), "{bad:?}");
        }
        for bad in ["", "0x", "10", "0xzz", "0x-1"] {
            assert!(Uint256::from_hex(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn scaled_is_exact_or_an_error() {
        let wei = Uint256::from_decimal("1000000000000000001").unwrap();
        assert_eq!(wei.scaled(18).unwrap().to_string(), "1.000000000000000001");
        assert_eq!(
            Uint256::from_decimal("1500000")
                .unwrap()
                .scaled(6)
                .unwrap()
                .to_string(),
            "1.5"
        );
        assert_eq!(
            Uint256::from_decimal("7")
                .unwrap()
                .scaled(0)
                .unwrap()
                .to_string(),
            "7"
        );
        assert_eq!(
            Uint256::from_decimal("5")
                .unwrap()
                .scaled(18)
                .unwrap()
                .to_string(),
            "0.000000000000000005"
        );
        // 2^256 - 1 does not fit 96 bits: refused, not rounded.
        assert!(
            Uint256::from_decimal(MAX_DECIMAL)
                .unwrap()
                .scaled(18)
                .is_err()
        );
        // 30 significant digits do not fit either.
        let thirty = Uint256::from_decimal(&"9".repeat(30)).unwrap();
        assert!(thirty.scaled(18).is_err());
    }
}
