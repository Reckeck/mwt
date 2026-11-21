//! The capital checkup (plan C4): the figures the light is judged from and the
//! texts of its notice. The light itself, the report and the scenarios are in
//! `usecases::planning::health`, because they name the debt, emergency and
//! insight types of other areas.

use chrono::{Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::error::{Error, Result};
use crate::model::planning::{HealthColor, HealthReasonCode};
use crate::money::books::Books;

/// DTI above this (percent) is red.
pub const DTI_HIGH_PCT: u32 = 50;
/// DTI above this (percent) is yellow.
pub const DTI_ELEVATED_PCT: u32 = 30;
/// A fund under this many months of mandatory spending is red.
pub const FUND_MIN_MONTHS: u32 = 1;
/// A fund under this many months is yellow.
pub const FUND_SAFE_MONTHS: u32 = 3;
/// The savings-rate chart's length, finished months.
pub(crate) const SAVINGS_MONTHS: usize = 12;
/// Finished months the savings rate and creep are judged over.
pub(crate) const RECENT_MONTHS: usize = 3;
/// The savings rate the chart points to (PDF 2.1), percent.
pub const SAVINGS_BENCHMARK_PCT: u32 = 20;

/// One reason for the light, with what its first step needs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct HealthReason {
    pub code: HealthReasonCode,
    /// The colour this reason alone would give; `Green` marks a hint.
    pub level: HealthColor,
    /// What the step is about: the debt, the gap to the fund level, the
    /// monthly shortfall, the spending growth.
    #[serde(default)]
    pub amount: Option<Amount>,
    /// The rate, the DTI, the months covered, the savings rate.
    #[serde(default)]
    pub pct: Option<Amount>,
    /// The loan's store id.
    #[serde(default)]
    pub subject: Option<String>,
}

/// The savings rate of one finished month.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct SavingsMonth {
    /// `YYYY-MM`.
    pub month: String,
    pub income: Amount,
    /// `income − fixed − daily`.
    pub saved: Amount,
    /// `saved / income`, percent; `None` without income.
    pub saved_pct: Option<Amount>,
}

/// `count` finished month starts before the month of `today`, oldest first.
pub(crate) fn finished_months(today: NaiveDate, count: usize) -> Vec<NaiveDate> {
    let mut month = calendar::first_of_month(today);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        month = calendar::first_of_month(month - Duration::days(1));
        out.push(month);
    }
    out.reverse();
    out
}

/// The same month `years` back.
pub(crate) fn years_back(month: NaiveDate, years: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(
        chrono::Datelike::year(&month) - years,
        chrono::Datelike::month(&month),
        1,
    )
    .unwrap_or(month)
}

pub(crate) fn percent(part: Amount, whole: Amount) -> Option<Amount> {
    (whole.is_positive()).then(|| (part / whole * Amount::from(100u32)).round(1))
}

/// Average income and spending (`fixed + daily`) of the given months;
/// `None` when any of them had no flow at all.
pub(crate) fn months_average(
    books: &Books<'_>,
    months: &[NaiveDate],
    zone: &str,
) -> Option<(Amount, Amount)> {
    let mut income = Amount::ZERO;
    let mut spent = Amount::ZERO;
    for month in months {
        let summary =
            crate::planning::budget::month_summary(books, &calendar::month_key(*month), zone)
                .ok()?;
        if summary.income.is_zero() && summary.fixed.is_zero() && summary.daily.is_zero() {
            return None;
        }
        income += summary.income;
        spent += summary.fixed + summary.daily;
    }
    let count = Amount::from(months.len() as u32);
    (!months.is_empty()).then(|| (income / count, spent / count))
}

/// The savings rate of the last finished months, oldest first.
pub fn savings_series(books: &Books<'_>, now: i64, zone: &str) -> Result<Vec<SavingsMonth>> {
    let today = Calendar::new(zone).date_of(now);
    finished_months(today, SAVINGS_MONTHS)
        .into_iter()
        .map(|month| {
            let summary =
                crate::planning::budget::month_summary(books, &calendar::month_key(month), zone)?;
            Ok(SavingsMonth {
                saved_pct: percent(summary.savings_fact, summary.income),
                month: summary.month,
                income: summary.income,
                saved: summary.savings_fact,
            })
        })
        .collect()
}

/// Saved against income over the last months with income, percent, and the
/// average monthly shortfall against `rate` (the planned share of income).
pub fn savings_figures(savings: &[SavingsMonth], rate: Amount) -> (Option<Amount>, Option<Amount>) {
    let recent: Vec<&SavingsMonth> = savings
        .iter()
        .rev()
        .filter(|month| month.income.is_positive())
        .take(RECENT_MONTHS)
        .collect();
    let income: Amount = recent
        .iter()
        .fold(Amount::ZERO, |sum, month| sum + month.income);
    let saved: Amount = recent
        .iter()
        .fold(Amount::ZERO, |sum, month| sum + month.saved);
    let gap = (!recent.is_empty()).then(|| {
        let plan = income * rate;
        ((plan - saved).max(Amount::ZERO) / Amount::from(recent.len() as u32)).round(2)
    });
    (percent(saved, income), gap)
}

/// Refuse a yearly return the freedom potential cannot assume.
pub fn check_freedom_rate(rate: Option<Amount>) -> Result<()> {
    if rate.is_some_and(|rate| rate.is_negative()) {
        return Err(Error::InvalidInput(
            "expected return: must not be negative".into(),
        ));
    }
    Ok(())
}

pub(crate) fn color_text(color: HealthColor, ru: bool) -> &'static str {
    match (color, ru) {
        (HealthColor::NoData, true) => "мало данных",
        (HealthColor::NoData, false) => "not enough data",
        (HealthColor::Green, true) => "зелёный",
        (HealthColor::Green, false) => "green",
        (HealthColor::Yellow, true) => "жёлтый",
        (HealthColor::Yellow, false) => "yellow",
        (HealthColor::Red, true) => "красный",
        (HealthColor::Red, false) => "red",
    }
}

pub(crate) fn reason_text(code: HealthReasonCode, ru: bool) -> &'static str {
    use HealthReasonCode as Code;
    match (code, ru) {
        (Code::NoHistory, true) => "Нет истории за полный месяц",
        (Code::NoHistory, false) => "No full month of history yet",
        (Code::CostlyLoan, true) => "Есть дорогой кредит",
        (Code::CostlyLoan, false) => "There is a costly loan",
        (Code::LoanWithoutRate, true) => "У кредита не указана ставка",
        (Code::LoanWithoutRate, false) => "A loan has no rate",
        (Code::DtiHigh, true) => "Платежи по кредитам больше половины дохода",
        (Code::DtiHigh, false) => "Loan payments take over half of income",
        (Code::DtiElevated, true) => "Платежи по кредитам больше 30% дохода",
        (Code::DtiElevated, false) => "Loan payments take over 30% of income",
        (Code::FundNotSet, true) => "Подушка безопасности не выбрана",
        (Code::FundNotSet, false) => "No emergency fund chosen",
        (Code::FundBelowOne, true) => "Подушки не хватит и на месяц",
        (Code::FundBelowOne, false) => "The emergency fund covers less than a month",
        (Code::FundBelowThree, true) => "Подушка меньше трёх месяцев",
        (Code::FundBelowThree, false) => "The emergency fund covers less than three months",
        (Code::SavingsBelowPlan, true) => "Откладываете меньше плана",
        (Code::SavingsBelowPlan, false) => "Saving less than planned",
        (Code::LifestyleCreep, true) => "Расходы растут не медленнее дохода",
        (Code::LifestyleCreep, false) => "Spending grows as fast as income",
    }
}
