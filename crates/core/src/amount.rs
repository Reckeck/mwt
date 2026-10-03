//! `Amount` — a decimal wrapper that is (de)serialized as a **string**
//! in both snapshot formats (JSON and bincode) and exported to TypeScript
//! as `string`. This avoids any float precision loss for money/quantities.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Amount(pub Decimal);

impl Amount {
    pub const ZERO: Amount = Amount(Decimal::ZERO);

    pub fn new(d: Decimal) -> Self {
        Amount(d)
    }

    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(Amount::ZERO);
        }
        let d = Decimal::from_str_exact(s)
            .map_err(|_| Error::InvalidInput(format!("bad number: {s}")))?;
        Ok(Amount(d))
    }

    pub fn from_f64(v: f64) -> Self {
        Amount(Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO))
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_negative(&self) -> bool {
        self.0.is_sign_negative()
    }

    /// Strictly greater than zero.
    pub fn is_positive(&self) -> bool {
        self.0.is_sign_positive() && !self.0.is_zero()
    }

    pub fn abs(self) -> Self {
        Amount(self.0.abs())
    }

    pub fn max(self, other: Self) -> Self {
        if self >= other { self } else { other }
    }

    pub fn to_f64(self) -> f64 {
        self.0.to_f64().unwrap_or(0.0)
    }

    pub fn round(self, dp: u32) -> Self {
        Amount(self.0.round_dp(dp))
    }

    /// Internal, unchecked access.
    pub fn into_inner(self) -> Decimal {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Arithmetic
// ---------------------------------------------------------------------------

impl Add for Amount {
    type Output = Amount;
    fn add(self, rhs: Amount) -> Amount {
        Amount(self.0 + rhs.0)
    }
}

impl AddAssign for Amount {
    fn add_assign(&mut self, rhs: Amount) {
        self.0 += rhs.0;
    }
}

impl Sub for Amount {
    type Output = Amount;
    fn sub(self, rhs: Amount) -> Amount {
        Amount(self.0 - rhs.0)
    }
}

impl SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Amount) {
        self.0 -= rhs.0;
    }
}

impl Neg for Amount {
    type Output = Amount;
    fn neg(self) -> Amount {
        Amount(-self.0)
    }
}

impl Mul for Amount {
    type Output = Amount;
    fn mul(self, rhs: Amount) -> Amount {
        Amount(self.0 * rhs.0)
    }
}

impl Div for Amount {
    type Output = Amount;
    fn div(self, rhs: Amount) -> Amount {
        Amount(self.0 / rhs.0)
    }
}

impl std::fmt::Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// serde — always a string on the wire / in the snapshot.
// ---------------------------------------------------------------------------

impl Serialize for Amount {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct AmountVisitor;
        impl Visitor<'_> for AmountVisitor {
            type Value = Amount;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a decimal number encoded as a string")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Amount, E> {
                Decimal::from_str_exact(v)
                    .map(Amount)
                    .map_err(|e| E::custom(format!("invalid decimal string `{v}`: {e}")))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Amount, E> {
                Ok(Amount::from_f64(v))
            }
        }
        deserializer.deserialize_str(AmountVisitor)
    }
}

impl ts_rs::TS for Amount {
    type WithoutGenerics = Self;

    fn name() -> String {
        "Amount".into()
    }

    fn ident() -> String {
        "Amount".into()
    }

    fn decl() -> String {
        "type Amount = string;".into()
    }

    fn decl_concrete() -> String {
        Self::decl()
    }

    fn inline() -> String {
        "string".into()
    }

    fn inline_flattened() -> String {
        Self::inline()
    }

    fn visit_dependencies(_: &mut impl ts_rs::TypeVisitor) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats() {
        let a = Amount::parse("123.456").unwrap();
        assert_eq!(a.to_string(), "123.456");
        assert!((a.round(2)).to_string().starts_with("123.46"));
    }

    #[test]
    fn negative_detection() {
        assert!(Amount::parse("-1.5").unwrap().is_negative());
        assert!(!Amount::parse("1.5").unwrap().is_negative());
    }

    #[test]
    fn arithmetic_precision() {
        let a = Amount::parse("0.1").unwrap();
        let b = Amount::parse("0.2").unwrap();
        assert_eq!((a + b).to_string(), "0.3");
    }
}
