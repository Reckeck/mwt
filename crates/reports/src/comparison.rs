//! A dashboard number with what it is measured against.

use serde::{Deserialize, Serialize};

use mwt_base::Amount;

/// What a dashboard number is measured against (plan H1, decision 57): the
/// plan where there is one, else the previous period up to the same date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum BaseKind {
    Plan,
    PrevPeriod,
}

/// A number with its base; `base` is `None` when there is nothing to compare.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Comparison {
    pub value: Amount,
    pub base: Option<Amount>,
    pub base_kind: BaseKind,
}

impl Comparison {
    pub fn new(value: Amount, base: Option<Amount>, base_kind: BaseKind) -> Self {
        Self {
            value,
            base,
            base_kind,
        }
    }

    /// `value − base`.
    pub fn delta(&self) -> Option<Amount> {
        self.base.map(|base| self.value - base)
    }

    /// The change in percent of the base; `None` on a zero base.
    pub fn delta_pct(&self) -> Option<Amount> {
        let base = self.base.filter(|base| !base.is_zero())?;
        let hundred = Amount::new(rust_decimal::Decimal::ONE_HUNDRED);
        Some(((self.value - base) / base.abs() * hundred).round(2))
    }
}

#[cfg(test)]
mod comparison_tests {
    use super::*;

    fn amount(text: &str) -> Amount {
        Amount::parse(text).unwrap()
    }

    #[test]
    fn delta_is_against_the_base() {
        let cmp = Comparison::new(amount("120"), Some(amount("100")), BaseKind::Plan);
        assert_eq!(cmp.delta(), Some(amount("20")));
        assert_eq!(cmp.delta_pct(), Some(amount("20")));
    }

    #[test]
    fn a_negative_base_keeps_the_sign_of_the_change() {
        let cmp = Comparison::new(amount("-50"), Some(amount("-100")), BaseKind::PrevPeriod);
        assert_eq!(cmp.delta_pct(), Some(amount("50")));
    }

    #[test]
    fn no_percent_on_a_missing_or_zero_base() {
        let zero = Comparison::new(amount("10"), Some(Amount::ZERO), BaseKind::PrevPeriod);
        assert_eq!(zero.delta(), Some(amount("10")));
        assert_eq!(zero.delta_pct(), None);
        let none = Comparison::new(amount("10"), None, BaseKind::PrevPeriod);
        assert_eq!(none.delta(), None);
    }
}
