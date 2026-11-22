//! The dashboard's "Today" strip and the calculator-scenario widget
//! (plan H4–H5): one request each, built from what the other widgets show.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use super::comparison::Comparison;
use super::dashboard_tiles::net_worth_trend;
use super::health::capital_health;
use mwt_base::Amount;
use mwt_base::calc::invest;
use mwt_base::calendar::{Calendar, clamp_day, first_of_month, month_key};
use mwt_base::error::{Error, Result};
use mwt_household::utilities::utility_bill;
use mwt_model::DashPeriod;
use mwt_model::HealthColor;
use mwt_model::StoreScope;
use mwt_money::books::Books;
use mwt_money::tree::{find_store, resolve_scope};
use mwt_planning::agenda::UpcomingKind;
use mwt_planning::daily_budget::daily_budget;
use mwt_planning::goals::store_values_usd;
use mwt_planning::scenarios::{find_calc_scenario, planned_balance};
use mwt_planning::upcoming::upcoming;

/// The strip lists at most this many things to do.
pub const TODAY_TODOS: usize = 3;
/// How far ahead the strip looks for payments and expiring things.
const TODAY_HORIZON_DAYS: u32 = 14;

/// What the strip asks the user to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TodayTodoKind {
    /// A payment the accounts will not cover by its date.
    Shortfall,
    /// An automation run that failed and waits for "Retry / Skip".
    RejectedRun,
    /// An operation booked with an approximate amount, waiting for the real one.
    UnconfirmedRun,
    /// Operations a workflow queued for confirmation.
    QueuedWorkflow,
    /// Exchange fills fetched but not yet taken into the ledger.
    PendingImport,
    /// Bonus points that burn soon.
    PointsExpire,
    /// A car's insurance that ends soon.
    InsuranceEnds,
    /// A property's utilities not paid this month, past its pay day.
    UtilityOverdue,
    /// A reminder due today or already past.
    Reminder,
}

/// One thing to do, with what it is about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TodayTodo {
    pub kind: TodayTodoKind,
    /// `YYYY-MM-DD` it is due; `None` sorts first: it is due already.
    pub date: Option<String>,
    /// Automation, item, store or queue name.
    pub title: String,
    /// The store, run, operation or queue it is about.
    pub source_id: String,
    /// Missing money, points or operations count, by kind.
    pub amount: Option<Amount>,
}

/// The strip above the dashboard grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DashboardToday {
    pub period: DashPeriod,
    /// Base currency of `net_worth`.
    pub currency: String,
    /// The capital now against the period's first day.
    pub net_worth: Comparison,
    /// The checkup's light; `None` when the checkup cannot be built.
    pub health: Option<HealthColor>,
    /// "Safe to spend today", in `budget_currency`.
    pub left_today: Amount,
    pub budget_currency: String,
    /// Up to three, the earliest first.
    pub todos: Vec<TodayTodo>,
    /// How many things there are in all, `todos` being the first of them.
    #[serde(default)]
    pub todos_total: u32,
}

/// A calculator scenario against what really happened (plan H4, decision 12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScenarioStatus {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub term_months: u32,
    /// Months passed since the scenario's start, at most its term.
    pub elapsed_months: u32,
    /// What the scenario ends with.
    pub final_nominal: Amount,
    /// What it plans to have by now.
    pub planned_now: Amount,
    /// What the scenario's wallets hold now; `None` when it has no wallets
    /// and no goal follows it.
    pub fact: Option<Amount>,
    /// The goal that follows the scenario, if any.
    pub goal_id: Option<String>,
}

/// Months from `start` (`YYYY-MM`) to the month of `today`; zero before it.
pub fn months_since(start: &str, today: NaiveDate) -> u32 {
    let Some(first) = NaiveDate::parse_from_str(&format!("{start}-01"), "%Y-%m-%d").ok() else {
        return 0;
    };
    let span = (today.year() - first.year()) * 12 + today.month() as i32 - first.month() as i32;
    span.max(0) as u32
}

/// Value of the wallets of `scope` in `currency`.
fn scope_value_in(books: &Books, scope: &StoreScope, currency: &str, now: i64) -> Option<Amount> {
    let values = store_values_usd(books);
    let usd = resolve_scope(books.doc(), scope)
        .iter()
        .filter_map(|id| values.get(id))
        .fold(Amount::ZERO, |sum, value| sum + *value);
    books.rate_book().convert(usd, "usd", currency, now)
}

/// Plan against fact for one saved scenario: the fact is its "From your data"
/// wallets, or else the wallets of the goal that follows it.
pub fn scenario_status(books: &Books, id: &str, now: i64, zone: &str) -> Result<ScenarioStatus> {
    let scenario = find_calc_scenario(books.doc(), id)
        .ok_or_else(|| Error::Missing(format!("scenario {id}")))?;
    let input = &scenario.input;
    let today = Calendar::new(zone).date_of(now);
    let elapsed = months_since(&input.start, today).min(input.term_months);
    let result = invest::simulate(input)?;
    let planned_now = planned_balance(input, elapsed)?;
    let goal = books
        .doc()
        .goals
        .iter()
        .find(|goal| goal.active && goal.scenario_id.as_deref() == Some(id));
    let scope = scenario
        .source
        .as_ref()
        .map(|source| &source.scope)
        .or(goal.map(|goal| &goal.scope));
    let fact = scope
        .and_then(|scope| scope_value_in(books, scope, &input.currency, now))
        .map(|value| value.round(2));
    Ok(ScenarioStatus {
        id: scenario.id.clone(),
        name: scenario.name.clone(),
        currency: input.currency.clone(),
        term_months: input.term_months,
        elapsed_months: elapsed,
        final_nominal: result.final_nominal,
        planned_now: planned_now.round(2),
        fact,
        goal_id: goal.map(|goal| goal.id.clone()),
    })
}

/// Everything waiting for the user, the earliest first; utility sums in `currency`.
pub fn all_today_todos(
    books: &Books,
    now: i64,
    zone: &str,
    currency: Option<&str>,
) -> Vec<TodayTodo> {
    let today = Calendar::new(zone).date_of(now);
    let horizon = today + Duration::days(i64::from(TODAY_HORIZON_DAYS));
    let within = |date: &str| {
        NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok_and(|day| day >= today && day <= horizon)
    };
    let mut todos = Vec::new();
    for item in upcoming(books, TODAY_HORIZON_DAYS, now, zone) {
        let kind = match item.kind {
            UpcomingKind::RejectedRun => TodayTodoKind::RejectedRun,
            UpcomingKind::UnconfirmedRun => TodayTodoKind::UnconfirmedRun,
            UpcomingKind::Reminder if item.date <= today.format("%Y-%m-%d").to_string() => {
                TodayTodoKind::Reminder
            }
            _ if item.shortfall.is_some() => TodayTodoKind::Shortfall,
            _ => continue,
        };
        todos.push(TodayTodo {
            kind,
            date: Some(item.date),
            title: item.title,
            source_id: item.source_id,
            amount: item.shortfall.or(item.amount.map(|value| value.abs())),
        });
    }
    let names: BTreeMap<&str, &str> = books
        .doc()
        .workflows
        .iter()
        .map(|flow| (flow.id.as_str(), flow.name.as_str()))
        .collect();
    for queued in &books.doc().workflow_queue {
        todos.push(TodayTodo {
            kind: TodayTodoKind::QueuedWorkflow,
            date: None,
            title: names
                .get(queued.workflow_id.as_str())
                .map_or_else(|| queued.summary.clone(), |name| (*name).to_owned()),
            source_id: queued.workflow_id.clone(),
            amount: Some(Amount::from(
                u32::try_from(queued.transactions.len()).unwrap_or(u32::MAX),
            )),
        });
    }
    let mut pending: BTreeMap<&str, u32> = BTreeMap::new();
    for fill in &books.doc().pending_imports {
        *pending.entry(fill.store_id.as_str()).or_default() += 1;
    }
    for (store_id, count) in pending {
        todos.push(TodayTodo {
            kind: TodayTodoKind::PendingImport,
            date: None,
            title: find_store(books.doc(), store_id)
                .map_or_else(String::new, |store| store.name.clone()),
            source_id: store_id.to_owned(),
            amount: Some(Amount::from(count)),
        });
    }
    for store in books.doc().stores.iter().filter(|store| !store.archived) {
        if let Some(info) = &store.loyalty
            && let Some(date) = info.expires_at.as_deref().filter(|date| within(date))
        {
            todos.push(TodayTodo {
                kind: TodayTodoKind::PointsExpire,
                date: Some(date.to_owned()),
                title: store.name.clone(),
                source_id: store.id.clone(),
                amount: info.expiring_points,
            });
        }
        let car = store.property.as_ref().and_then(|info| info.car.as_ref());
        if let Some(date) = car
            .into_iter()
            .flat_map(|car| [car.osago_until.as_deref(), car.kasko_until.as_deref()])
            .flatten()
            .filter(|date| within(date))
            .min()
        {
            todos.push(TodayTodo {
                kind: TodayTodoKind::InsuranceEnds,
                date: Some(date.to_owned()),
                title: store.name.clone(),
                source_id: store.id.clone(),
                amount: None,
            });
        }
    }
    todos.extend(overdue_utilities(books, now, zone, currency));
    // Undated things are due already; the sort is stable, so the order
    // above breaks ties.
    todos.sort_by(|left, right| left.date.cmp(&right.date));
    todos
}

/// Properties whose pay day has passed this month with nothing paid for some
/// utility: the unpaid sum, in `currency` when every item converts to it.
fn overdue_utilities(
    books: &Books,
    now: i64,
    zone: &str,
    currency: Option<&str>,
) -> Vec<TodayTodo> {
    let today = Calendar::new(zone).date_of(now);
    let rates = books.rate_book();
    let mut todos = Vec::new();
    for store in books.doc().stores.iter().filter(|store| !store.archived) {
        let Some(day) = store
            .property
            .as_ref()
            .and_then(|info| info.utilities_pay_day)
        else {
            continue;
        };
        let due = clamp_day(first_of_month(today), day);
        if due >= today {
            continue;
        }
        let Ok(bill) = utility_bill(books, &store.id, &month_key(today), zone, now) else {
            continue;
        };
        let unpaid: Vec<_> = bill
            .lines
            .iter()
            .filter(|line| line.paid.is_zero())
            .collect();
        if unpaid.is_empty() {
            continue;
        }
        let amount = currency.and_then(|to| {
            unpaid.iter().try_fold(Amount::ZERO, |sum, line| {
                let value = line.amount?;
                Some(sum + rates.convert(value, &line.item.currency, to, now)?)
            })
        });
        todos.push(TodayTodo {
            kind: TodayTodoKind::UtilityOverdue,
            date: Some(due.format("%Y-%m-%d").to_string()),
            title: store.name.clone(),
            source_id: store.id.clone(),
            amount: amount.map(|value| value.round(2)),
        });
    }
    todos
}

/// Everything "Today" shows, in one request (plan H5).
pub fn dashboard_today(books: &Books, period: DashPeriod, now: i64, zone: &str) -> DashboardToday {
    let trend = net_worth_trend(books, period, now, zone);
    let budget = daily_budget(books, now, zone);
    let todos = all_today_todos(books, now, zone, Some(&budget.currency));
    DashboardToday {
        period,
        currency: trend.currency,
        net_worth: trend.change,
        health: capital_health(books, now, zone)
            .ok()
            .map(|report| report.color),
        left_today: budget.left_today,
        todos_total: u32::try_from(todos.len()).unwrap_or(u32::MAX),
        todos: todos.into_iter().take(TODAY_TODOS).collect(),
        budget_currency: budget.currency,
    }
}
