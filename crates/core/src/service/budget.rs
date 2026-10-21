//! Budgets, the daily budget and month totals (plan §7.3).
//!
//! Everything here reads the same thing: money flows of the budget stores
//! (`in_budget`), classified once by [`UnlockedVault::classify_flow`] and
//! valued in the budget currency at the rate of the day they happened (D20).
//! Days, weeks and months are cut in the user's local zone (§7.6).

use std::collections::HashSet;

use chrono::{Datelike, Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, AUTOMATION_HOUR, Calendar, days_in_month, first_of_month};
use crate::error::{Error, Result};
use crate::model::planning::{AutomationOutcome, Budget, BudgetPeriod, BudgetTarget, UtilityKind};
use crate::model::store::{StoreKind, StoreScope};
use crate::model::transaction::{Leg, LegRole, Transaction};
use crate::model::vault::DailyBudgetMode;
use crate::pricing::history::secs_or_now;

use super::automations::draft_to_transaction;
use super::rates::RateBook;
use super::{FlowClass, UnlockedVault};

/// What a budget is: everything the user edits.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetDraft {
    pub active: bool,
    pub target: BudgetTarget,
    pub period: BudgetPeriod,
    pub limit: Amount,
    pub currency: String,
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

/// How far back [`UnlockedVault::avg_month`] looks for months with flows.
const AVG_LOOKBACK_MONTHS: usize = 24;
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

/// One classified flow.
#[derive(Debug, Clone)]
pub(crate) struct Flow {
    pub date: NaiveDate,
    pub class: FlowClass,
    /// In the requested currency.
    pub value: Amount,
    pub categories: Vec<String>,
    pub stores: Vec<String>,
    pub priced: bool,
}

fn one() -> Amount {
    Amount::new(rust_decimal::Decimal::ONE)
}

fn amount_of_days(days: u32) -> Amount {
    Amount::new(rust_decimal::Decimal::from(days))
}

impl UnlockedVault {
    /// Whether everyday spending under these categories is mandatory: the
    /// first known category decides, as it decides the flow's group.
    fn is_essential_spending(&self, categories: &[String]) -> bool {
        categories
            .iter()
            .find_map(|key| {
                self.doc
                    .categories
                    .iter()
                    .find(|category| &category.key == key)
            })
            .is_some_and(|category| category.is_essential())
    }

    /// The budget currency, never empty (D20).
    pub fn budget_currency(&self) -> String {
        let code = self.doc.settings.budget_currency.trim().to_lowercase();
        if code.is_empty() {
            self.base_and_fx().0
        } else {
            code
        }
    }

    /// Value of one leg in `currency` at `secs`; `None` without a rate.
    pub(crate) fn leg_value_in(
        book: &RateBook,
        leg: &Leg,
        currency: &str,
        secs: i64,
    ) -> Option<Amount> {
        let qty = leg.amount.abs();
        if leg.asset.is_fiat() && leg.asset.symbol.eq_ignore_ascii_case(currency) {
            return Some(qty);
        }
        let usd = leg
            .price_usd
            .or_else(|| book.price_at(&leg.asset.key(), secs))?;
        book.convert(qty * usd, "usd", currency, secs)
    }

    /// What a flow is worth to the budget: the money that came in for income
    /// and money entering the budget, the money (and fees) that went out for
    /// everything else.
    fn flow_value(
        book: &RateBook,
        tx: &Transaction,
        class: FlowClass,
        currency: &str,
    ) -> (Amount, bool) {
        let secs = secs_or_now(&tx.meta().timestamp);
        let incoming = matches!(class, FlowClass::Income | FlowClass::BudgetIn);
        let mut total = Amount::ZERO;
        let mut priced = true;
        for leg in tx.legs() {
            let counts = match leg.role {
                LegRole::Principal => leg.amount.is_positive() == incoming,
                LegRole::Fee => !incoming,
                // A correction is neither income nor spending.
                LegRole::Pnl | LegRole::Adjustment => false,
            };
            if !counts || leg.amount.is_zero() {
                continue;
            }
            match Self::leg_value_in(book, &leg, currency, secs) {
                Some(value) => total += value,
                None => priced = false,
            }
        }
        (total, priced)
    }

    /// Classified flows in `[from, to)`, valued in `currency`.
    pub(crate) fn flows_between(
        &self,
        cal: &Calendar,
        currency: &str,
        from: i64,
        to: i64,
    ) -> Vec<Flow> {
        let book = self.rate_book();
        self.doc
            .transactions
            .iter()
            .filter_map(|tx| {
                let secs = secs_or_now(&tx.meta().timestamp);
                if secs < from || secs >= to {
                    return None;
                }
                let class = self.classify_flow(tx);
                if class == FlowClass::Ignored {
                    return None;
                }
                let (value, priced) = Self::flow_value(&book, tx, class, currency);
                Some(Flow {
                    date: cal.date_of(secs),
                    class,
                    value,
                    categories: tx.meta().categories.clone(),
                    stores: tx.legs().into_iter().map(|leg| leg.store_id).collect(),
                    priced,
                })
            })
            .collect()
    }

    // -- CRUD ---------------------------------------------------------------

    pub fn budgets(&self) -> Vec<Budget> {
        self.doc.budgets.clone()
    }

    fn check_budget(&self, draft: &BudgetDraft) -> Result<()> {
        if !draft.limit.is_positive() {
            return Err(Error::InvalidInput("the limit must be positive".into()));
        }
        if draft.currency.trim().is_empty() {
            return Err(Error::InvalidInput("choose a currency".into()));
        }
        match &draft.target {
            BudgetTarget::Total => Ok(()),
            BudgetTarget::Category(key) => {
                if self
                    .doc
                    .categories
                    .iter()
                    .any(|category| &category.key == key)
                {
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
                    .find(|id| self.find_store(id).is_none())
                {
                    Some(id) => Err(Error::Missing(format!("store {id}"))),
                    None => Ok(()),
                }
            }
        }
    }

    pub fn create_budget(&mut self, draft: BudgetDraft) -> Result<Budget> {
        self.check_budget(&draft)?;
        let budget = Budget {
            id: uuid::Uuid::new_v4().to_string(),
            active: draft.active,
            target: draft.target,
            period: draft.period,
            limit: draft.limit,
            currency: draft.currency.trim().to_lowercase(),
        };
        self.doc.budgets.push(budget.clone());
        self.persist()?;
        Ok(budget)
    }

    pub fn update_budget(&mut self, id: &str, draft: BudgetDraft) -> Result<Budget> {
        self.check_budget(&draft)?;
        let budget = self
            .doc
            .budgets
            .iter_mut()
            .find(|budget| budget.id == id)
            .ok_or_else(|| Error::Missing(format!("budget {id}")))?;
        budget.active = draft.active;
        budget.target = draft.target;
        budget.period = draft.period;
        budget.limit = draft.limit;
        budget.currency = draft.currency.trim().to_lowercase();
        let out = budget.clone();
        self.persist()?;
        Ok(out)
    }

    pub fn delete_budget(&mut self, id: &str) -> Result<()> {
        let before = self.doc.budgets.len();
        self.doc.budgets.retain(|budget| budget.id != id);
        if self.doc.budgets.len() == before {
            return Err(Error::Missing(format!("budget {id}")));
        }
        self.persist()
    }

    // -- limits -------------------------------------------------------------

    /// `[start, end)` dates of the period containing `today`; weeks start on Monday.
    fn period_dates(period: BudgetPeriod, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        match period {
            BudgetPeriod::Day => (today, today + Duration::days(1)),
            BudgetPeriod::Week => {
                let start =
                    today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
                (start, start + Duration::days(7))
            }
            BudgetPeriod::Month => {
                let start = first_of_month(today);
                (start, calendar::next_month(start))
            }
        }
    }

    /// Every active limit with what its current period used (plan §7.3):
    /// mandatory and everyday spending of the budget stores, by category, by
    /// wallet or in total, in the limit's own currency at each day's rate.
    pub fn budget_status(&self, now: i64, zone: &str) -> Vec<BudgetStatus> {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        self.doc
            .budgets
            .iter()
            .filter(|budget| budget.active)
            .map(|budget| {
                let (start, end) = Self::period_dates(budget.period, today);
                let scope: Option<HashSet<String>> = match &budget.target {
                    BudgetTarget::Store(scope) => Some(self.resolve_scope(scope)),
                    _ => None,
                };
                let spent = self
                    .flows_between(
                        &cal,
                        &budget.currency,
                        cal.start_of(start),
                        cal.start_of(end),
                    )
                    .into_iter()
                    .filter(|flow| matches!(flow.class, FlowClass::Fixed | FlowClass::Daily))
                    .filter(|flow| match &budget.target {
                        BudgetTarget::Total => true,
                        BudgetTarget::Category(key) => flow.categories.contains(key),
                        BudgetTarget::Store(_) => scope
                            .as_ref()
                            .is_some_and(|ids| flow.stores.iter().any(|id| ids.contains(id))),
                    })
                    .fold(Amount::ZERO, |sum, flow| sum + flow.value);
                let used = if budget.limit.is_zero() {
                    Amount::ZERO
                } else {
                    spent / budget.limit
                };
                BudgetStatus {
                    budget: budget.clone(),
                    spent,
                    remaining: budget.limit - spent,
                    used,
                    period_start: calendar::iso(cal.start_of(start)),
                    period_end: calendar::iso(cal.start_of(end)),
                }
            })
            .collect()
    }

    // -- month totals -------------------------------------------------------

    /// Income, mandatory spending, everyday spending and savings of a calendar
    /// month (`YYYY-MM`) in the budget currency (plan §7.3).
    pub fn month_summary(&self, month: &str, zone: &str) -> Result<MonthSummary> {
        let first = calendar::parse_month(month)
            .ok_or_else(|| Error::InvalidInput(format!("month {month}: expected YYYY-MM")))?;
        let cal = Calendar::new(zone);
        let (from, to) = cal.month_bounds(first);
        let currency = self.budget_currency();
        let mut out = MonthSummary {
            month: calendar::month_key(first),
            currency: currency.clone(),
            complete: true,
            ..MonthSummary::default()
        };
        for flow in self.flows_between(&cal, &currency, from, to) {
            out.complete &= flow.priced;
            match flow.class {
                FlowClass::Income => out.income += flow.value,
                FlowClass::Fixed => out.fixed += flow.value,
                FlowClass::Daily => {
                    out.daily += flow.value;
                    if self.is_essential_spending(&flow.categories) {
                        out.essential_daily += flow.value;
                    }
                }
                FlowClass::Savings => out.savings_moved += flow.value,
                FlowClass::BudgetIn => out.budget_in += flow.value,
                FlowClass::BudgetOut => out.budget_out += flow.value,
                FlowClass::Ignored => {}
            }
        }
        out.savings_plan = out.income * self.doc.settings.savings_rate;
        out.savings_fact = out.income - out.fixed - out.daily;
        out.deviation = out.savings_fact - out.savings_plan;
        if out.income.is_positive() {
            out.saved_pct =
                (out.savings_fact / out.income * Amount::parse("100").unwrap_or(one())).round(1);
        }
        out.not_executed = self
            .automation_runs(None)
            .iter()
            .filter(|run| !matches!(run.outcome, AutomationOutcome::Created { .. }))
            .filter(|run| {
                let secs = secs_or_now(&run.planned_at);
                secs >= from && secs < to
            })
            .count() as u32;
        Ok(out)
    }

    // -- the plan of a month from automations ---------------------------------

    /// Planned income and mandatory spending of the month containing `date`:
    /// active automations by their class, rent (`Housing`) and loan payments
    /// that no automation already covers.
    pub(crate) fn month_plan(
        &self,
        cal: &Calendar,
        date: NaiveDate,
        currency: &str,
    ) -> (Amount, Amount) {
        let book = self.rate_book();
        let first = first_of_month(date);
        let last = calendar::next_month(first) - Duration::days(1);
        let mut income = Amount::ZERO;
        let mut fixed = Amount::ZERO;
        let mut paid_loans: HashSet<String> = HashSet::new();
        for automation in self.automations().iter().filter(|item| item.active) {
            // Moved off days off: a payday on the 1st may land in the month before.
            let dates = self
                .automation_occurrences(
                    automation,
                    cal,
                    cal.start_of(first) - 1,
                    cal.start_of(last + Duration::days(1)) - 1,
                )
                .len();
            let probe = draft_to_transaction(
                &self.automation_draft(automation),
                &automation.id,
                "",
                &calendar::iso(cal.at_local(first, AUTOMATION_HOUR)),
            );
            if let Transaction::Transfer(inner) = &probe
                && let Some(target) = inner.to.store_id()
            {
                paid_loans.insert(target.to_string());
            }
            if dates == 0 {
                continue;
            }
            let class = self.classify_flow(&probe);
            let (value, _) = Self::flow_value(&book, &probe, class, currency);
            let total = value * amount_of_days(dates as u32);
            match class {
                FlowClass::Income => income += total,
                FlowClass::Fixed => fixed += total,
                _ => {}
            }
        }
        for item in self.doc.utility_items.iter().filter(|item| item.active) {
            if let UtilityKind::Housing {
                amount,
                automation_id: None,
                ..
            } = &item.kind
            {
                fixed += book
                    .convert(*amount, &item.currency, currency, cal.start_of(first))
                    .unwrap_or(Amount::ZERO);
            }
        }
        for store in &self.doc.stores {
            if store.kind != StoreKind::Liability
                || store.archived
                || paid_loans.contains(&store.id)
            {
                continue;
            }
            let Some(payment) = store
                .liability
                .as_ref()
                .and_then(|terms| terms.monthly_payment)
            else {
                continue;
            };
            let code = self.store_currency(&store.id);
            fixed += book
                .convert(payment, &code, currency, cal.start_of(first))
                .unwrap_or(payment);
        }
        (income, fixed)
    }

    /// The currency a store mostly holds, for terms entered without one:
    /// its largest fiat holding, else the base currency.
    pub(crate) fn store_currency(&self, store_id: &str) -> String {
        self.doc
            .holdings
            .iter()
            .filter(|holding| holding.store_id == store_id && holding.asset.is_fiat())
            .max_by(|left, right| left.quantity.abs().cmp(&right.quantity.abs()))
            .map(|holding| holding.asset.symbol.to_lowercase())
            .unwrap_or_else(|| self.base_and_fx().0)
    }

    /// The user's own month from the settings (plan B): income and spending,
    /// `months = 0`. `None` without a hand-entered income.
    pub fn manual_month(&self) -> Option<MonthAvg> {
        let settings = &self.doc.settings;
        let income = settings.manual_income.filter(|value| value.is_positive())?;
        let expenses = settings.manual_expenses.unwrap_or(Amount::ZERO);
        Some(MonthAvg {
            income,
            fixed: expenses,
            daily: Amount::ZERO,
            essential: expenses,
            months: 0,
            currency: self.budget_currency(),
        })
    }

    /// [`Self::avg_month`], or the hand-entered month while fewer than
    /// [`MIN_HISTORY_MONTHS`] finished months have flows.
    pub fn income_basis(&self, months: usize, now: i64, zone: &str) -> Option<MonthAvg> {
        let average = self.avg_month(months, now, zone);
        if average.as_ref().map_or(0, |avg| avg.months) < MIN_HISTORY_MONTHS
            && let Some(manual) = self.manual_month()
        {
            return Some(manual);
        }
        average
    }

    /// Averages of the last `months` finished calendar months that had any
    /// flow, before the month of `now`; `None` without such months.
    pub fn avg_month(&self, months: usize, now: i64, zone: &str) -> Option<MonthAvg> {
        let cal = Calendar::new(zone);
        let mut month = first_of_month(cal.date_of(now));
        let mut found: Vec<MonthSummary> = Vec::new();
        for _ in 0..AVG_LOOKBACK_MONTHS {
            if found.len() >= months {
                break;
            }
            month = first_of_month(month - Duration::days(1));
            let summary = self.month_summary(&calendar::month_key(month), zone).ok()?;
            if summary.income.is_zero() && summary.fixed.is_zero() && summary.daily.is_zero() {
                continue;
            }
            found.push(summary);
        }
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
            currency: self.budget_currency(),
        })
    }

    // -- the daily budget ---------------------------------------------------

    /// How much can be spent today (plan §7.3), in the budget currency.
    pub fn daily_budget(&self, now: i64, zone: &str) -> DailyBudget {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let currency = self.budget_currency();
        let first = first_of_month(today);
        let (from, to) = cal.month_bounds(today);
        let flows = self.flows_between(&cal, &currency, from, to);
        let spent_on = |date: NaiveDate| {
            flows
                .iter()
                .filter(|flow| flow.class == FlowClass::Daily && flow.date == date)
                .fold(Amount::ZERO, |sum, flow| sum + flow.value)
        };
        let spent_today = spent_on(today);
        let manual_limit = self.manual_day_limit(&currency, now);
        let mode = self.doc.settings.daily_budget_mode;
        let mut out = DailyBudget {
            mode,
            currency: currency.clone(),
            date: today.format("%Y-%m-%d").to_string(),
            per_day: Amount::ZERO,
            carry: Amount::ZERO,
            available: Amount::ZERO,
            spent_today,
            left_today: Amount::ZERO,
            income: Amount::ZERO,
            fixed: Amount::ZERO,
            savings_plan: Amount::ZERO,
            free: Amount::ZERO,
            days: 0,
            next_income: None,
            manual_limit,
        };
        match mode {
            DailyBudgetMode::Month => {
                let allowance = self.month_allowance(&cal, today, &currency, &flows);
                out.income = allowance.income;
                out.fixed = allowance.fixed;
                out.savings_plan = allowance.savings_plan;
                out.days = allowance.days;
                out.per_day = allowance.per_day;
                let mut day = first;
                while day < today {
                    out.carry += out.per_day - spent_on(day);
                    day += Duration::days(1);
                }
                out.available = out.per_day + out.carry;
            }
            DailyBudgetMode::UntilIncome => {
                let next_income = self.next_income_date(&cal, today);
                let end = next_income.unwrap_or_else(|| calendar::next_month(first));
                out.next_income = next_income.map(|date| date.format("%Y-%m-%d").to_string());
                out.days = (end - today).num_days().max(1) as u32;
                let due = self.fixed_due_between(&cal, today, end, &currency);
                out.free = self.budget_balance(&currency, now) - due;
                // The allowance is fixed for the day: what was spent today is
                // already out of the balance, so add it back.
                out.per_day = (out.free + spent_today) / amount_of_days(out.days);
                out.available = out.per_day;
            }
        }
        if let Some(limit) = manual_limit {
            out.available = out.available.min(limit);
        }
        out.left_today = out.available - spent_today;
        out
    }

    /// The month-mode allowance of the month containing `date` (plan §7.3):
    /// income and mandatory spending — the larger of fact and plan — less the
    /// savings plan, spread over the month's days. `flows` are that month's.
    pub(crate) fn month_allowance(
        &self,
        cal: &Calendar,
        date: NaiveDate,
        currency: &str,
        flows: &[Flow],
    ) -> MonthAllowance {
        let fact = |class: FlowClass| {
            flows
                .iter()
                .filter(|flow| flow.class == class)
                .fold(Amount::ZERO, |sum, flow| sum + flow.value)
        };
        let (plan_income, plan_fixed) = self.month_plan(cal, date, currency);
        let income = fact(FlowClass::Income).max(plan_income);
        let fixed = fact(FlowClass::Fixed).max(plan_fixed);
        let savings_plan = income * self.doc.settings.savings_rate;
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
    pub(crate) fn manual_day_limit(&self, currency: &str, now: i64) -> Option<Amount> {
        let book = self.rate_book();
        self.doc
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
    fn budget_balance(&self, currency: &str, now: i64) -> Amount {
        let book = self.rate_book();
        let ids: HashSet<&str> = self
            .doc
            .stores
            .iter()
            .filter(|store| store.in_budget() && !store.archived)
            .map(|store| store.id.as_str())
            .collect();
        self.doc
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

    /// The first planned income date after `today` from active automations.
    fn next_income_date(&self, cal: &Calendar, today: NaiveDate) -> Option<NaiveDate> {
        let horizon = today + Duration::days(366);
        self.automations()
            .iter()
            .filter(|item| item.active)
            .filter(|automation| {
                let probe = draft_to_transaction(&automation.template, "", "", "");
                self.classify_flow(&probe) == FlowClass::Income
            })
            .filter_map(|automation| {
                self.automation_occurrences(
                    automation,
                    cal,
                    cal.start_of(today + Duration::days(1)) - 1,
                    cal.start_of(horizon + Duration::days(1)) - 1,
                )
                .first()
                .map(|(secs, _)| cal.date_of(*secs))
            })
            .min()
    }

    /// Mandatory payments planned on `[today, end)`: fixed automations, rent
    /// and loan payments by their due day.
    fn fixed_due_between(
        &self,
        cal: &Calendar,
        today: NaiveDate,
        end: NaiveDate,
        currency: &str,
    ) -> Amount {
        let book = self.rate_book();
        let last = end - Duration::days(1);
        let mut total = Amount::ZERO;
        let mut paid_loans: HashSet<String> = HashSet::new();
        for automation in self.automations().iter().filter(|item| item.active) {
            let probe = draft_to_transaction(&self.automation_draft(automation), "", "", "");
            if let Transaction::Transfer(inner) = &probe
                && let Some(target) = inner.to.store_id()
            {
                paid_loans.insert(target.to_string());
            }
            if self.classify_flow(&probe) != FlowClass::Fixed {
                continue;
            }
            let count = self
                .automation_occurrences(
                    automation,
                    cal,
                    cal.start_of(today) - 1,
                    cal.start_of(end) - 1,
                )
                .len();
            let (value, _) = Self::flow_value(&book, &probe, FlowClass::Fixed, currency);
            total += value * amount_of_days(count as u32);
        }
        let due_in_window = |due_day: u8| {
            let mut month = first_of_month(today);
            let mut hits = 0u32;
            while month <= last {
                let date = calendar::clamp_day(month, due_day);
                if date >= today && date <= last {
                    hits += 1;
                }
                month = calendar::next_month(month);
            }
            hits
        };
        for item in self.doc.utility_items.iter().filter(|item| item.active) {
            if let UtilityKind::Housing {
                amount,
                due_day,
                automation_id: None,
                ..
            } = &item.kind
            {
                let value = book
                    .convert(*amount, &item.currency, currency, cal.start_of(today))
                    .unwrap_or(Amount::ZERO);
                total += value * amount_of_days(due_in_window(*due_day));
            }
        }
        for store in &self.doc.stores {
            if store.kind != StoreKind::Liability
                || store.archived
                || paid_loans.contains(&store.id)
            {
                continue;
            }
            let Some(terms) = store.liability.as_ref() else {
                continue;
            };
            if let (Some(payment), Some(due_day)) = (terms.monthly_payment, terms.due_day) {
                let code = self.store_currency(&store.id);
                let value = book
                    .convert(payment, &code, currency, cal.start_of(today))
                    .unwrap_or(payment);
                total += value * amount_of_days(due_in_window(due_day));
            }
        }
        total
    }

    /// Whether a scope belongs to the budget — used by the form's over-limit hint.
    pub fn scope_in_budget(&self, scope: &StoreScope) -> bool {
        self.resolve_scope(scope)
            .iter()
            .any(|id| self.find_store(id).is_some_and(|store| store.in_budget()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::planning::Schedule;
    use crate::model::planning::TransactionDraft;
    use crate::model::store::{AssetRef, LiabilityKind, LiabilityTerms};
    use crate::model::transaction::{AccountRef, PriceStamp};
    use crate::model::vault::FxPoint;
    use crate::pricing::history::unix_secs;
    use crate::service::AutomationDraft;
    use crate::service::StoreExtras;
    use crate::service::test_support::*;

    fn secs(iso: &str) -> i64 {
        unix_secs(iso).unwrap()
    }

    fn dated(mut tx: Transaction, ts: &str) -> Transaction {
        at(&mut tx, ts);
        tx
    }

    fn spend(store: &str, sum: &str, ts: &str) -> Transaction {
        dated(payment(store, sum, "food"), ts)
    }

    /// USD between two stores of the user's.
    fn usd_move(from: &str, to: &str, sum: &str, ts: &str) -> Transaction {
        dated(
            transfer_between(
                AccountRef::Store(from.into()),
                AccountRef::Store(to.into()),
                usd(),
                sum,
            ),
            ts,
        )
    }

    fn earn(store: &str, sum: &str, ts: &str) -> Transaction {
        dated(income(store, sum, "salary"), ts)
    }

    /// A bank card in the budget, USD everywhere.
    fn budget_vault() -> (tempfile::TempDir, UnlockedVault, String) {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        (dir, vault, card)
    }

    #[test]
    fn month_summary_counts_income_fixed_and_daily_but_not_internal_or_debts() {
        let (_dir, mut vault, card) = budget_vault();
        let pocket = store(&mut vault, "Pocket", StoreKind::Bank, Some(&card));
        vault
            .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
            .unwrap();
        let mut rent = spend(&card, "300", "2026-03-02T10:00:00Z");
        rent.meta_mut().categories = vec!["rent_mortgage".into()];
        vault.add_transaction(rent).unwrap();
        vault
            .add_transaction(spend(&card, "200", "2026-03-03T10:00:00Z"))
            .unwrap();
        // Internal and debt moves are neither income nor spending.
        let inner = usd_move(&card, &pocket, "100", "2026-03-04T10:00:00Z");
        vault.add_transaction(inner).unwrap();
        let friend = vault
            .create_contact(crate::service::ContactDraft {
                group: String::new(),
                name: "Friend".into(),
                kind: crate::model::contact::ContactKind::Person,
                note: String::new(),
                accounts: Vec::new(),
            })
            .unwrap()
            .id;
        let mut lend = transfer_between(
            AccountRef::Store(card.clone()),
            AccountRef::Contact {
                contact_id: friend,
                account_id: None,
            },
            usd(),
            "50",
        );
        if let Transaction::Transfer(tx) = &mut lend {
            tx.debt = Some(crate::model::transaction::DebtEffect::Lend);
        }
        at(&mut lend, "2026-03-05T10:00:00Z");
        vault.add_transaction(lend).unwrap();
        // Another month does not count.
        vault
            .add_transaction(spend(&card, "999", "2026-04-01T10:00:00Z"))
            .unwrap();

        let summary = vault.month_summary("2026-03", "UTC").unwrap();
        assert_eq!(summary.income, amount("1000"));
        assert_eq!(summary.fixed, amount("300"));
        assert_eq!(summary.daily, amount("200"));
        assert_eq!(summary.savings_plan, amount("100"));
        assert_eq!(summary.savings_fact, amount("500"));
        assert_eq!(summary.deviation, amount("400"));
        assert_eq!(summary.saved_pct, amount("50"));
    }

    #[test]
    fn avg_month_takes_finished_months_with_flows_only() {
        let (_dir, mut vault, card) = budget_vault();
        let now = secs("2026-05-15T10:00:00Z");
        assert_eq!(vault.avg_month(3, now, "UTC"), None);
        for (sum, ts) in [
            ("900", "2026-01-10T10:00:00Z"),
            ("1000", "2026-02-10T10:00:00Z"),
            ("1200", "2026-04-10T10:00:00Z"),
            // The current month is not finished yet.
            ("5000", "2026-05-01T10:00:00Z"),
        ] {
            vault.add_transaction(earn(&card, sum, ts)).unwrap();
        }
        vault
            .add_transaction(spend(&card, "300", "2026-04-11T10:00:00Z"))
            .unwrap();

        // March had no flows and is skipped: April, February.
        let two = vault.avg_month(2, now, "UTC").unwrap();
        assert_eq!(two.months, 2);
        assert_eq!(two.income, amount("1100"));
        assert_eq!(two.daily, amount("150"));
        assert_eq!(two.currency, "usd");
        // Fewer months than asked: what there is.
        let all = vault.avg_month(12, now, "UTC").unwrap();
        assert_eq!(all.months, 3);
        assert_eq!(all.income, amount("1033.3333333333333333333333333"));
    }

    #[test]
    fn a_loan_payment_is_mandatory_and_exchange_to_card_tops_up_the_budget() {
        let (_dir, mut vault, card) = budget_vault();
        let loan = vault
            .create_store_with(
                "Mortgage",
                StoreKind::Liability,
                "",
                "",
                None,
                StoreExtras {
                    liability: Some(LiabilityTerms {
                        kind: LiabilityKind::Mortgage,
                        rate: None,
                        limit: None,
                        due_day: Some(5),
                        monthly_payment: Some(amount("400")),
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        let exchange = store(&mut vault, "Exchange", StoreKind::Exchange, None);
        vault
            .add_transaction(earn(&exchange, "2000", "2026-03-01T10:00:00Z"))
            .unwrap();
        let cash_out = usd_move(&exchange, &card, "1500", "2026-03-02T10:00:00Z");
        vault.add_transaction(cash_out).unwrap();
        let repay = usd_move(&card, &loan, "400", "2026-03-05T10:00:00Z");
        vault.add_transaction(repay).unwrap();

        let summary = vault.month_summary("2026-03", "UTC").unwrap();
        assert_eq!(summary.income, Amount::ZERO);
        assert_eq!(summary.budget_in, amount("1500"));
        assert_eq!(summary.fixed, amount("400"));
    }

    #[test]
    fn month_mode_carries_what_was_left_and_resets_next_month() {
        let (_dir, mut vault, card) = budget_vault();
        vault.doc.settings.savings_rate = Amount::ZERO;
        // 3100 income in a 31-day month: 100 a day.
        vault
            .add_transaction(earn(&card, "3100", "2026-03-01T08:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "40", "2026-03-01T12:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "180", "2026-03-02T12:00:00Z"))
            .unwrap();

        let day3 = vault.daily_budget(secs("2026-03-03T12:00:00Z"), "UTC");
        assert_eq!(day3.per_day, amount("100"));
        // +60 left on the 1st, −80 overspent on the 2nd.
        assert_eq!(day3.carry, amount("-20"));
        assert_eq!(day3.available, amount("80"));

        let april = vault.daily_budget(secs("2026-04-01T12:00:00Z"), "UTC");
        assert_eq!(april.carry, Amount::ZERO);
        assert_eq!(april.income, Amount::ZERO);
    }

    #[test]
    fn planned_income_counts_until_the_salary_arrives_and_savings_rate_applies() {
        let (_dir, mut vault, card) = budget_vault();
        let Transaction::Topup(salary) = income(&card, "3000", "salary") else {
            unreachable!()
        };
        vault
            .create_automation(AutomationDraft {
                name: "Salary".into(),
                active: true,
                preset: None,
                schedule: Schedule::Monthly { day: 25 },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template: TransactionDraft::Topup(salary),
                amount_mode: Default::default(),
                workday_shift: Default::default(),
            })
            .unwrap();
        let early = vault.daily_budget(secs("2026-06-03T12:00:00Z"), "UTC");
        assert_eq!(early.income, amount("3000"));
        assert_eq!(early.savings_plan, amount("300"));
        assert_eq!(early.per_day, amount("90"));

        vault.doc.settings.savings_rate = Amount::ZERO;
        vault
            .add_transaction(earn(&card, "4500", "2026-06-04T08:00:00Z"))
            .unwrap();
        let after = vault.daily_budget(secs("2026-06-05T12:00:00Z"), "UTC");
        assert_eq!(after.income, amount("4500"));
    }

    #[test]
    fn until_income_mode_without_salary_spreads_to_month_end() {
        let (_dir, mut vault, card) = budget_vault();
        vault.doc.settings.daily_budget_mode = DailyBudgetMode::UntilIncome;
        vault
            .add_transaction(earn(&card, "1100", "2026-03-01T08:00:00Z"))
            .unwrap();
        // The 21st: 11 days left in March.
        let today = vault.daily_budget(secs("2026-03-21T12:00:00Z"), "UTC");
        assert_eq!(today.days, 11);
        assert_eq!(today.per_day, amount("100"));
        assert_eq!(today.next_income, None);
    }

    #[test]
    fn spending_off_budget_stores_does_not_touch_the_daily_budget() {
        let (_dir, mut vault, card) = budget_vault();
        let exchange = store(&mut vault, "Exchange", StoreKind::Exchange, None);
        vault
            .add_transaction(earn(&card, "3100", "2026-03-01T08:00:00Z"))
            .unwrap();
        vault
            .add_transaction(earn(&exchange, "500", "2026-03-01T08:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&exchange, "300", "2026-03-01T12:00:00Z"))
            .unwrap();
        let day = vault.daily_budget(secs("2026-03-01T13:00:00Z"), "UTC");
        assert_eq!(day.spent_today, Amount::ZERO);
    }

    #[test]
    fn the_day_is_cut_at_local_midnight() {
        let (_dir, mut vault, card) = budget_vault();
        vault
            .add_transaction(earn(&card, "3100", "2026-03-01T08:00:00Z"))
            .unwrap();
        // 20:00 UTC on the 1st is 03:00 on the 2nd in Bangkok.
        vault
            .add_transaction(spend(&card, "50", "2026-03-01T20:00:00Z"))
            .unwrap();
        let utc = vault.daily_budget(secs("2026-03-01T21:00:00Z"), "UTC");
        let bangkok = vault.daily_budget(secs("2026-03-01T21:00:00Z"), "Asia/Bangkok");
        assert_eq!(utc.spent_today, amount("50"));
        assert_eq!(bangkok.date, "2026-03-02");
        assert_eq!(bangkok.spent_today, amount("50"));
        let bangkok_first = vault.daily_budget(secs("2026-03-01T15:00:00Z"), "Asia/Bangkok");
        assert_eq!(bangkok_first.spent_today, Amount::ZERO);
    }

    #[test]
    fn a_rub_limit_with_a_usd_base_and_a_thb_budget_use_the_day_rate() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        vault.doc.settings.budget_currency = "thb".into();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let rub = AssetRef::fiat("RUB", "Ruble");
        let fx = |code: &str, rate: &str, ts: &str| FxPoint {
            currency: code.into(),
            per_usd: amount(rate),
            ts: ts.into(),
            source: "test".into(),
        };
        // Mark RUB as used so it keeps history.
        let mut seed = income_of(&card, rub.clone(), "10000", "0.01", "seed");
        at(&mut seed, "2026-01-01T00:00:00Z");
        vault.add_transaction(seed).unwrap();
        vault.apply_fx(&[
            fx("rub", "100", "2026-03-01T00:00:00Z"),
            fx("thb", "40", "2026-03-01T00:00:00Z"),
        ]);
        vault.apply_fx(&[
            fx("rub", "50", "2026-04-01T00:00:00Z"),
            fx("thb", "20", "2026-04-01T00:00:00Z"),
        ]);

        let mut groceries = payment(&card, "1000", "food");
        if let Transaction::Payment(inner) = &mut groceries {
            inner.asset = rub.clone();
            inner.price = PriceStamp::default();
        }
        groceries.meta_mut().categories = vec!["groceries".into()];
        at(&mut groceries, "2026-03-10T10:00:00Z");
        vault.add_transaction(groceries).unwrap();

        // March rate: 1000 RUB = 10 USD = 400 THB, even though April's rates are newer.
        let march = vault.month_summary("2026-03", "UTC").unwrap();
        assert_eq!(march.currency, "thb");
        assert_eq!(march.daily, amount("400"));

        vault
            .create_budget(BudgetDraft {
                active: true,
                target: BudgetTarget::Category("groceries".into()),
                period: BudgetPeriod::Month,
                limit: amount("5000"),
                currency: "rub".into(),
            })
            .unwrap();
        let status = vault.budget_status(secs("2026-03-20T00:00:00Z"), "UTC");
        assert_eq!(status[0].spent, amount("1000"));
        assert_eq!(status[0].remaining, amount("4000"));
    }
}
