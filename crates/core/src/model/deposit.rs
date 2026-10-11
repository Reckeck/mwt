//! Interest-bearing money: savings accounts, term deposits and the
//! long-term savings programme (ПДС) with its state co-financing.
//!
//! Rates are entered by the user — no public source publishes a bank's own
//! rate — and kept as a history, so a rate change does not rewrite what was
//! earned before it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;

/// A rate in force from a day on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct RatePeriod {
    /// `YYYY-MM-DD`.
    pub from: String,
    /// Annual rate, percent.
    pub rate: Amount,
}

/// When earned interest joins the balance.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capitalization {
    /// Paid out, never compounded.
    None,
    Daily,
    #[default]
    Monthly,
    /// Once, when a term deposit ends.
    AtEnd,
}

/// Monthly income bracket of the ПДС co-financing ratio.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IncomeBracket {
    /// Up to 80 000 ₽ a month: the state adds 1 ₽ per 1 ₽.
    #[default]
    Low,
    /// 80 000 – 150 000 ₽: 1 ₽ per 2 ₽.
    Mid,
    /// Above 150 000 ₽: 1 ₽ per 4 ₽.
    High,
}

/// The long-term savings programme (ПДС): state co-financing of
/// contributions for ten years from the first one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct PdsTerms {
    /// `YYYY-MM-DD` of the first contribution; the ten years run from it.
    pub first_contribution_at: String,
    pub income_bracket: IncomeBracket,
}

/// Terms of a savings account or a term deposit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct DepositTerms {
    /// Oldest first; the last one with `from` ≤ a day is that day's rate.
    pub rates: Vec<RatePeriod>,
    #[serde(default)]
    pub capitalization: Capitalization,
    /// Term deposits: `YYYY-MM-DD` it was opened and how long it runs.
    #[serde(default)]
    pub opened_at: Option<String>,
    #[serde(default)]
    pub term_months: Option<u32>,
    /// Book the month's interest as a top-up on the 1st of the next month.
    #[serde(default)]
    pub auto_accrue: bool,
    #[serde(default)]
    pub pds: Option<PdsTerms>,
}

impl DepositTerms {
    /// The rate in force on `day` (`YYYY-MM-DD`), percent.
    pub fn rate_on(&self, day: &str) -> Option<Amount> {
        self.rates
            .iter()
            .filter(|period| period.from.as_str() <= day)
            .max_by(|left, right| left.from.cmp(&right.from))
            .map(|period| period.rate)
    }
}

/// The Bank of Russia key rate from a day on, percent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct KeyRatePoint {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub rate: Amount,
}
