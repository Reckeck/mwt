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

    pub fn new(value: Decimal) -> Self {
        Amount(value)
    }

    pub fn parse(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Amount::ZERO);
        }
        Decimal::from_str_exact(trimmed)
            .map(Amount)
            .map_err(|_| Error::InvalidInput(format!("bad number: {trimmed}")))
    }

    pub fn from_f64(value: f64) -> Self {
        Amount(Decimal::from_f64_retain(value).unwrap_or(Decimal::ZERO))
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

    /// `self^exp` by squaring; `None` on overflow. The `maths` feature of
    /// `rust_decimal` is off, so powers are built from multiplication.
    pub fn checked_pow_int(self, exp: u32) -> Option<Self> {
        let mut base = self.0;
        let mut acc = Decimal::ONE;
        let mut rest = exp;
        while rest > 0 {
            if rest & 1 == 1 {
                acc = acc.checked_mul(base)?;
            }
            rest >>= 1;
            if rest > 0 {
                base = base.checked_mul(base)?;
            }
        }
        Some(Amount(acc))
    }

    /// `self^exp`, saturating at the largest decimal on overflow.
    pub fn pow_int(self, exp: u32) -> Self {
        self.checked_pow_int(exp)
            .unwrap_or(Amount(if self.is_negative() && exp % 2 == 1 {
                Decimal::MIN
            } else {
                Decimal::MAX
            }))
    }

    /// The positive `root`-th root by Newton's method; zero for a non-positive
    /// value or `root == 0`. Exact to the 28 digits a decimal holds for the
    /// growth factors it is used on (`1.07^(1/12)` and the like).
    pub fn root_n(self, root: u32) -> Self {
        if !self.is_positive() || root == 0 {
            return Amount::ZERO;
        }
        if root == 1 {
            return self;
        }
        let degree = Decimal::from(root);
        let lower = Decimal::from(root - 1);
        // Near one (the usual case) a first-order guess converges in a few steps.
        let mut guess = if self.0 < Decimal::TWO {
            Decimal::ONE + (self.0 - Decimal::ONE) / degree
        } else {
            self.0 / degree + Decimal::ONE
        };
        for _ in 0..50 {
            let Some(power) = Amount(guess).checked_pow_int(root - 1) else {
                guess /= Decimal::TWO;
                continue;
            };
            if power.is_zero() {
                break;
            }
            let next = (lower * guess + self.0 / power.0) / degree;
            let done = (next - guess).abs() < Decimal::new(1, 26);
            guess = next;
            if done {
                break;
            }
        }
        Amount(guess)
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

impl From<u32> for Amount {
    fn from(value: u32) -> Self {
        Amount(Decimal::from(value))
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

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a decimal number encoded as a string")
            }

            fn visit_str<E: serde::de::Error>(self, raw: &str) -> std::result::Result<Amount, E> {
                Decimal::from_str_exact(raw)
                    .map(Amount)
                    .map_err(|err| E::custom(format!("invalid decimal string `{raw}`: {err}")))
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> std::result::Result<Amount, E> {
                Ok(Amount::from_f64(value))
            }
        }
        deserializer.deserialize_str(AmountVisitor)
    }
}

impl ts_rs::TS for Amount {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_cfg: &ts_rs::Config) -> String {
        "Amount".into()
    }

    fn ident(_cfg: &ts_rs::Config) -> String {
        "Amount".into()
    }

    fn decl(_cfg: &ts_rs::Config) -> String {
        "type Amount = string;".into()
    }

    fn decl_concrete(cfg: &ts_rs::Config) -> String {
        <Self as ts_rs::TS>::decl(cfg)
    }

    fn inline(_cfg: &ts_rs::Config) -> String {
        "string".into()
    }

    fn inline_flattened(cfg: &ts_rs::Config) -> String {
        <Self as ts_rs::TS>::inline(cfg)
    }

    fn visit_dependencies(_: &mut impl ts_rs::TypeVisitor) {}
}

/// Schema for the editor: `Amount` reaches JSON as a decimal *string*, so the
/// schema must say `string` rather than expose the `Decimal` newtype.
impl schemars::JsonSchema for Amount {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "string".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <String as schemars::JsonSchema>::json_schema(generator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats() {
        let parsed = Amount::parse("123.456").unwrap();
        assert_eq!(parsed.to_string(), "123.456");
        assert!((parsed.round(2)).to_string().starts_with("123.46"));
    }

    #[test]
    fn negative_detection() {
        assert!(Amount::parse("-1.5").unwrap().is_negative());
        assert!(!Amount::parse("1.5").unwrap().is_negative());
    }

    #[test]
    fn integer_powers() {
        let base = Amount::parse("1.02").unwrap();
        assert_eq!(base.pow_int(0), Amount::parse("1").unwrap());
        assert_eq!(base.pow_int(1), base);
        assert_eq!(
            base.pow_int(8).round(10),
            Amount::parse("1.1716593810").unwrap()
        );
        assert_eq!(
            Amount::parse("2").unwrap().pow_int(10),
            Amount::parse("1024").unwrap()
        );
        assert!(Amount::parse("10").unwrap().checked_pow_int(40).is_none());
        assert_eq!(
            Amount::parse("10").unwrap().pow_int(40),
            Amount(Decimal::MAX)
        );
    }

    #[test]
    fn nth_roots() {
        let monthly = Amount::parse("1.1").unwrap().root_n(12);
        assert_eq!(
            monthly.round(20),
            Amount::parse("1.00797414042890374107").unwrap()
        );
        assert_eq!(monthly.pow_int(12).round(18), Amount::parse("1.1").unwrap());
        assert_eq!(
            Amount::parse("1024").unwrap().root_n(10).round(18),
            Amount::parse("2").unwrap()
        );
        assert_eq!(
            Amount::parse("1.5")
                .unwrap()
                .root_n(1200)
                .pow_int(1200)
                .round(12),
            Amount::parse("1.5").unwrap()
        );
        assert_eq!(
            Amount::parse("0.81").unwrap().root_n(2).round(18),
            Amount::parse("0.9").unwrap()
        );
        assert_eq!(Amount::ZERO.root_n(12), Amount::ZERO);
        assert_eq!(Amount::parse("-4").unwrap().root_n(2), Amount::ZERO);
        assert_eq!(Amount::parse("4").unwrap().root_n(0), Amount::ZERO);
    }

    #[test]
    fn arithmetic_precision() {
        let tenth = Amount::parse("0.1").unwrap();
        let fifth = Amount::parse("0.2").unwrap();
        assert_eq!((tenth + fifth).to_string(), "0.3");
    }
}
