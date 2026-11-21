//! Budgets, the daily budget and month totals (plan §7.3): the types and the
//! rules that need only the document or the books of a request.
//!
//! The figures of a period come from money flows of the budget stores
//! (`in_budget`), classified once by `classify_flow` and valued in the budget
//! currency at the rate of the day they happened (D20). The flows carry the
//! worth classification (`planning::flows`). Days, weeks and months are cut
//! in the user's local zone (§7.6). Nothing here reads the clock: `now` is an
//! argument.

use std::collections::HashSet;

use chrono::{Datelike, Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, Calendar, days_in_month, first_of_month};
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::model::planning::{
    AutomationOutcome, AutomationRun, Budget, BudgetPeriod, BudgetTarget,
};
use crate::model::store::StoreScope;
use crate::model::vault::{DailyBudgetMode, VaultDoc};
use crate::money::worth::FlowClass;
use crate::time::secs_or_now;

use super::flows::flows_between;

use super::goals::resolve_scope;

/// What a budget is: everything the user edits.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetDraft {
    pub active: bool,
    pub target: BudgetTarget,
    pub period: BudgetPeriod,
    pub limit: Amount,
    pub currency: String,
    pub warn: bool,
}

/// A limit and how much of it the current period used.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct BudgetStatus {
    pub budget: Budget,
    /// Spent in the current period, in the budget's own currency.
    pub spent: Amount,
    pub remaining: Amount,
    /// `spent / limit`, 0 when the limit is zero.
    pub used: Amount,
    pub period_start: String,
    pub period_end: String,
    /// Some spending of the period has no rate to the limit's currency, so
    /// `spent` leaves it out and is only a lower bound (C2-8). The front end
    /// ignores the field until a UI shows the mark.
    #[serde(default)]
    pub not_valued: bool,
}

/// Totals of one calendar month in the budget currency.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct MonthSummary {
    /// `YYYY-MM`.
    pub month: String,
    pub currency: String,
    pub income: Amount,
    pub fixed: Amount,
    pub daily: Amount,
    /// The part of `daily` in essential categories (plan C1).
    #[serde(default)]
    pub essential_daily: Amount,
    /// `income × savings_rate`.
    pub savings_plan: Amount,
    /// What was left: `income − fixed − daily`.
    pub savings_fact: Amount,
    /// `savings_fact − savings_plan`.
    pub deviation: Amount,
    /// Share of income left, percent; 0 without income.
    pub saved_pct: Amount,
    /// Money moved to savings and goal stores.
    pub savings_moved: Amount,
    /// Money that entered the budget from the user's other stores.
    pub budget_in: Amount,
    /// Money that left the budget for the user's other stores.
    pub budget_out: Amount,
    /// Automation runs planned this month that created nothing.
    pub not_executed: u32,
    /// Whether every flow had a rate; a missing rate counts as zero.
    pub complete: bool,
}

/// Monthly averages over finished calendar months, in the budget currency.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct MonthAvg {
    pub income: Amount,
    pub fixed: Amount,
    pub daily: Amount,
    /// Mandatory spending `E_mand`: `fixed` plus essential everyday spending.
    pub essential: Amount,
    /// How many months went into the average.
    pub months: u32,
    pub currency: String,
}

/// How far back the monthly average looks for months with flows.
pub(crate) const AVG_LOOKBACK_MONTHS: usize = 24;
/// Finished months of history before hand-entered income stops counting (plan B).
pub const MIN_HISTORY_MONTHS: u32 = 3;

/// The daily budget for today (plan §7.3, D18).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct DailyBudget {
    pub mode: DailyBudgetMode,
    pub currency: String,
    /// Local date, `YYYY-MM-DD`.
    pub date: String,
    /// The day's allowance by the mode's formula.
    pub per_day: Amount,
    /// Carry-over from the earlier days of the month (month mode); zero otherwise.
    pub carry: Amount,
    /// `per_day + carry`, or the hand-set day limit when it is smaller.
    pub available: Amount,
    pub spent_today: Amount,
    /// `available − spent_today`.
    pub left_today: Amount,
    /// Month mode: income, mandatory spending and the savings plan of the month.
    pub income: Amount,
    pub fixed: Amount,
    pub savings_plan: Amount,
    /// Until-income mode: budget balance minus what is due before the income.
    pub free: Amount,
    /// Days the allowance is spread over.
    pub days: u32,
    /// Date of the next planned income, until-income mode.
    pub next_income: Option<String>,
    /// A hand-set `Total / Day` limit in the budget currency, if any.
    pub manual_limit: Option<Amount>,
}

/// What the month mode allows per day of one month.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MonthAllowance {
    pub income: Amount,
    pub fixed: Amount,
    pub savings_plan: Amount,
    pub days: u32,
    pub per_day: Amount,
}

pub(crate) fn one() -> Amount {
    Amount::new(rust_decimal::Decimal::ONE)
}

pub(crate) fn amount_of_days(days: u32) -> Amount {
    Amount::new(rust_decimal::Decimal::from(days))
}

/// Whether everyday spending under these categories is mandatory: the
/// first known category decides, as it decides the flow's group.
pub(crate) fn is_essential_spending(doc: &VaultDoc, categories: &[String]) -> bool {
    categories
        .iter()
        .find_map(|key| doc.categories.iter().find(|category| &category.key == key))
        .is_some_and(|category| category.is_essential())
}

/// The budget currency, never empty (D20).
pub fn budget_currency(books: &Books<'_>) -> String {
    let code = books.doc().settings.budget_currency.trim().to_lowercase();
    if code.is_empty() {
        books.base_and_fx().0.to_string()
    } else {
        code
    }
}

/// The currency a store mostly holds, for terms entered without one:
/// its largest fiat holding, else the base currency.
pub(crate) fn store_currency(books: &Books<'_>, store_id: &str) -> String {
    books
        .doc()
        .holdings
        .iter()
        .filter(|holding| holding.store_id == store_id && holding.asset.is_fiat())
        .max_by(|left, right| left.quantity.abs().cmp(&right.quantity.abs()))
        .map(|holding| holding.asset.symbol.to_lowercase())
        .unwrap_or_else(|| books.base_and_fx().0.to_string())
}

pub fn budgets(doc: &VaultDoc) -> Vec<Budget> {
    doc.budgets.clone()
}

/// Whether a scope belongs to the budget: the form's over-limit hint.
pub fn scope_in_budget(books: &Books<'_>, scope: &StoreScope) -> bool {
    resolve_scope(books, scope).iter().any(|id| {
        books
            .doc()
            .stores
            .iter()
            .any(|store| &store.id == id && store.in_budget())
    })
}

// -- CRUD -------------------------------------------------------------------

pub(crate) fn check_budget(doc: &VaultDoc, draft: &BudgetDraft) -> Result<()> {
    if !draft.limit.is_positive() {
        return Err(Error::InvalidInput("the limit must be positive".into()));
    }
    if draft.currency.trim().is_empty() {
        return Err(Error::InvalidInput("choose a currency".into()));
    }
    match &draft.target {
        BudgetTarget::Total => Ok(()),
        BudgetTarget::Category(key) => {
            if doc.categories.iter().any(|category| &category.key == key) {
                Ok(())
            } else {
                Err(Error::Missing(format!("category {key}")))
            }
        }
        BudgetTarget::Store(scope) => {
            if scope.store_ids.is_empty() {
                return Err(Error::InvalidInput("choose a wallet".into()));
            }
            match scope
                .store_ids
                .iter()
                .find(|id| !doc.stores.iter().any(|store| &store.id == *id))
            {
                Some(id) => Err(Error::Missing(format!("store {id}"))),
                None => Ok(()),
            }
        }
    }
}

/// The budget a draft makes, under the given id.
pub fn plan_create_budget(doc: &VaultDoc, draft: BudgetDraft, id: String) -> Result<Budget> {
    check_budget(doc, &draft)?;
    Ok(Budget {
        id,
        active: draft.active,
        target: draft.target,
        period: draft.period,
        limit: draft.limit,
        currency: draft.currency.trim().to_lowercase(),
        warn: draft.warn,
    })
}

/// The budget `id` as the draft changes it.
pub fn plan_update_budget(doc: &VaultDoc, id: &str, draft: BudgetDraft) -> Result<Budget> {
    check_budget(doc, &draft)?;
    if !doc.budgets.iter().any(|budget| budget.id == id) {
        return Err(Error::Missing(format!("budget {id}")));
    }
    plan_create_budget(doc, draft, id.to_string())
}

/// The keys of the notices a deleted limit takes with it: "exceeded" about
/// nothing is stale. Fails when there is no such budget.
pub fn plan_delete_budget(doc: &VaultDoc, id: &str) -> Result<Vec<String>> {
    if !doc.budgets.iter().any(|budget| budget.id == id) {
        return Err(Error::Missing(format!("budget {id}")));
    }
    let prefix = format!("budget:{id}:");
    Ok(doc
        .notification_log
        .iter()
        .filter(|record| record.key.starts_with(&prefix))
        .map(|record| record.key.clone())
        .collect())
}

// -- limits -----------------------------------------------------------------

/// `[start, end)` dates of the period containing `today`; weeks start on Monday.
pub(crate) fn period_dates(period: BudgetPeriod, today: NaiveDate) -> (NaiveDate, NaiveDate) {
    match period {
        BudgetPeriod::Day => (today, today + Duration::days(1)),
        BudgetPeriod::Week => {
            let start = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
            (start, start + Duration::days(7))
        }
        BudgetPeriod::Month => {
            let start = first_of_month(today);
            (start, calendar::next_month(start))
        }
    }
}

/// `spent / limit`, 0 when the limit is zero.
pub(crate) fn used_of(spent: Amount, limit: Amount) -> Amount {
    if limit.is_zero() {
        Amount::ZERO
    } else {
        spent / limit
    }
}

// -- month totals -----------------------------------------------------------

/// What follows from the flows of a month once they are summed: the savings
/// plan against the fact and the share of income left.
pub(crate) fn settle_month(out: &mut MonthSummary, savings_rate: Amount) {
    out.savings_plan = out.income * savings_rate;
    out.savings_fact = out.income - out.fixed - out.daily;
    out.deviation = out.savings_fact - out.savings_plan;
    if out.income.is_positive() {
        out.saved_pct =
            (out.savings_fact / out.income * Amount::parse("100").unwrap_or(one())).round(1);
    }
}

/// Automation runs planned in `[from, to)` that created nothing.
pub(crate) fn not_executed(runs: &[AutomationRun], from: i64, to: i64) -> u32 {
    runs.iter()
        .filter(|run| !matches!(run.outcome, AutomationOutcome::Created { .. }))
        .filter(|run| {
            let secs = secs_or_now(&run.planned_at);
            secs >= from && secs < to
        })
        .count() as u32
}

/// The user's own month from the settings (plan B): income and spending,
/// `months = 0`. `None` without a hand-entered income.
pub fn manual_month(books: &Books<'_>) -> Option<MonthAvg> {
    let settings = &books.doc().settings;
    let income = settings.manual_income.filter(|value| value.is_positive())?;
    let expenses = settings.manual_expenses.unwrap_or(Amount::ZERO);
    Some(MonthAvg {
        income,
        fixed: expenses,
        daily: Amount::ZERO,
        essential: expenses,
        months: 0,
        currency: budget_currency(books),
    })
}

/// Income, mandatory spending, everyday spending and savings of a calendar
/// month (`YYYY-MM`) in the budget currency (plan §7.3). `not_executed` stays
/// zero: it comes from the automation runs, which sit above this layer.
pub fn month_summary(books: &Books<'_>, month: &str, zone: &str) -> Result<MonthSummary> {
    let first = calendar::parse_month(month)
        .ok_or_else(|| Error::InvalidInput(format!("month {month}: expected YYYY-MM")))?;
    let cal = Calendar::new(zone);
    let (from, to) = cal.month_bounds(first);
    let currency = budget_currency(books);
    let mut out = MonthSummary {
        month: calendar::month_key(first),
        currency: currency.clone(),
        complete: true,
        ..MonthSummary::default()
    };
    for flow in flows_between(books, &cal, &currency, from, to) {
        out.complete &= flow.priced();
        match flow.class {
            FlowClass::Income => out.income += flow.value,
            FlowClass::Fixed => out.fixed += flow.value,
            FlowClass::Daily => {
                out.daily += flow.value;
                if is_essential_spending(books.doc(), &flow.categories) {
                    out.essential_daily += flow.value;
                }
            }
            FlowClass::Savings => out.savings_moved += flow.value,
            FlowClass::BudgetIn => out.budget_in += flow.value,
            FlowClass::BudgetOut => out.budget_out += flow.value,
            FlowClass::Ignored => {}
        }
    }
    settle_month(&mut out, books.doc().settings.savings_rate);
    Ok(out)
}

/// Averages of the last `months` finished calendar months that had any
/// flow, before the month of `now`; `None` without such months.
pub fn avg_month(books: &Books<'_>, months: usize, now: i64, zone: &str) -> Option<MonthAvg> {
    let cal = Calendar::new(zone);
    let mut month = first_of_month(cal.date_of(now));
    let mut found: Vec<MonthSummary> = Vec::new();
    for _ in 0..AVG_LOOKBACK_MONTHS {
        if found.len() >= months {
            break;
        }
        month = first_of_month(month - Duration::days(1));
        let summary = month_summary(books, &calendar::month_key(month), zone).ok()?;
        if summary.income.is_zero() && summary.fixed.is_zero() && summary.daily.is_zero() {
            continue;
        }
        found.push(summary);
    }
    average_month(&found, budget_currency(books))
}

/// [`avg_month`], or the hand-entered month while fewer than
/// `MIN_HISTORY_MONTHS` finished months have flows.
pub fn income_basis(books: &Books<'_>, months: usize, now: i64, zone: &str) -> Option<MonthAvg> {
    income_basis_of(avg_month(books, months, now, zone), manual_month(books))
}

/// The average of month summaries; `None` without any.
pub(crate) fn average_month(found: &[MonthSummary], currency: String) -> Option<MonthAvg> {
    if found.is_empty() {
        return None;
    }
    let count = Amount::from(found.len() as u32);
    let average = |pick: fn(&MonthSummary) -> Amount| {
        found
            .iter()
            .fold(Amount::ZERO, |sum, summary| sum + pick(summary))
            / count
    };
    Some(MonthAvg {
        income: average(|summary| summary.income),
        fixed: average(|summary| summary.fixed),
        daily: average(|summary| summary.daily),
        essential: average(|summary| summary.fixed + summary.essential_daily),
        months: found.len() as u32,
        currency,
    })
}

/// The average, or the hand-entered month while fewer than
/// [`MIN_HISTORY_MONTHS`] finished months have flows.
pub(crate) fn income_basis_of(
    average: Option<MonthAvg>,
    manual: Option<MonthAvg>,
) -> Option<MonthAvg> {
    if average.as_ref().map_or(0, |avg| avg.months) < MIN_HISTORY_MONTHS
        && let Some(manual) = manual
    {
        return Some(manual);
    }
    average
}

// -- the daily budget -------------------------------------------------------

/// What the month mode allows per day: income and mandatory spending — the
/// larger of fact and plan — less the savings plan, spread over the days of
/// the month containing `date`.
pub(crate) fn allowance_of(
    fact: (Amount, Amount),
    plan: (Amount, Amount),
    savings_rate: Amount,
    date: NaiveDate,
) -> MonthAllowance {
    let income = fact.0.max(plan.0);
    let fixed = fact.1.max(plan.1);
    let savings_plan = income * savings_rate;
    let days = days_in_month(date);
    MonthAllowance {
        income,
        fixed,
        savings_plan,
        days,
        per_day: (income - fixed - savings_plan) / amount_of_days(days),
    }
}

/// The smallest active `Total / Day` limit, in `currency`.
pub(crate) fn manual_day_limit(books: &Books<'_>, currency: &str, now: i64) -> Option<Amount> {
    let book = books.rate_book();
    books
        .doc()
        .budgets
        .iter()
        .filter(|budget| {
            budget.active
                && budget.period == BudgetPeriod::Day
                && budget.target == BudgetTarget::Total
        })
        .filter_map(|budget| book.convert(budget.limit, &budget.currency, currency, now))
        .min()
}

/// Current value of the budget stores in `currency`.
pub(crate) fn budget_balance(books: &Books<'_>, currency: &str, now: i64) -> Amount {
    let book = books.rate_book();
    let ids: HashSet<&str> = books
        .doc()
        .stores
        .iter()
        .filter(|store| store.in_budget() && !store.archived)
        .map(|store| store.id.as_str())
        .collect();
    books
        .doc()
        .holdings
        .iter()
        .filter(|holding| ids.contains(holding.store_id.as_str()))
        .filter(|holding| !holding.asset.is_derivative())
        .filter_map(|holding| {
            if holding.asset.is_fiat() && holding.asset.symbol.eq_ignore_ascii_case(currency) {
                return Some(holding.quantity);
            }
            let usd = book.price_at(&holding.asset.key(), now)?;
            book.convert(holding.quantity * usd, "usd", currency, now)
        })
        .fold(Amount::ZERO, |sum, value| sum + value)
}
