//! Goals (plan §7.4, D17, D23): how much is saved, how much the user put
//! in, how much to put aside a month, and whether that is realistic.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate};

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::{Error, Result};
use crate::model::planning::{Envelope, Goal};
use crate::model::store::StoreScope;
use crate::model::transaction::LegRole;
use crate::model::vault::VaultDoc;
use crate::money::books::Books;
use crate::time::secs_or_now;

use super::scenarios::{find_calc_scenario, planned_balance};

/// How many non-empty past months the free surplus averages (plan §7.4).
pub const SURPLUS_MONTHS: usize = 6;
/// Days of history the pace forecast fits a trend to.
pub const FORECAST_DAYS: i64 = 90;
/// Average days in a month, to turn a daily pace into a monthly one.
pub const DAYS_PER_MONTH: f64 = 30.44;

/// What a goal is: everything the user edits.
#[derive(Debug, Clone, PartialEq)]
pub struct GoalDraft {
    pub name: String,
    pub active: bool,
    pub target_amount: Amount,
    pub currency: String,
    pub starts_at: String,
    pub deadline: Option<String>,
    pub priority: u8,
    pub monthly_commitment: Option<Amount>,
    pub scope: StoreScope,
    pub scenario_id: Option<String>,
    pub expected_return: Option<Amount>,
}

/// Highest expected return a goal accepts, percent a year.
const MAX_EXPECTED_RETURN: u32 = 100;
/// Months the forecast with a return looks ahead before giving up.
const RETURN_FORECAST_MONTHS: u32 = 1200;

/// What to put aside a month for `left` months to grow `saved` into
/// `target` at the monthly rate `monthly` (an annuity; linear at zero).
pub fn need_with_return(saved: Amount, target: Amount, monthly: Amount, left: u32) -> Amount {
    let months = Amount::from(left.max(1));
    if !monthly.is_positive() {
        return ((target - saved) / months).max(Amount::ZERO);
    }
    let growth = (Amount::from(1) + monthly).pow_int(left.max(1));
    let missing = target - saved * growth;
    if !missing.is_positive() {
        return Amount::ZERO;
    }
    missing * monthly / (growth - Amount::from(1))
}

/// Months until `saved` plus `per_month` a month, growing at `monthly`,
/// reaches `target`; `None` beyond [`RETURN_FORECAST_MONTHS`].
pub fn months_to_target(
    saved: Amount,
    target: Amount,
    monthly: Amount,
    per_month: Amount,
) -> Option<u32> {
    let growth = Amount::from(1) + monthly.max(Amount::ZERO);
    let mut balance = saved;
    for month in 0..=RETURN_FORECAST_MONTHS {
        if balance >= target {
            return Some(month);
        }
        balance = balance * growth + per_month;
    }
    None
}

/// Why a goal is marked "Revise".
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum RealismIssue {
    /// Needs more a month than the user said they can put aside.
    OverCommitment { need: Amount, commitment: Amount },
    /// All goals together need more a month than is usually left over;
    /// both amounts in the budget currency.
    OverSurplus {
        need_total: Amount,
        surplus: Amount,
        currency: String,
    },
}

/// A deleted goal and what went with it: enough to undo.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct GoalDeleted {
    pub goal: Goal,
    pub envelopes: Vec<Envelope>,
    /// Workflows switched off with it.
    pub workflow_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct GoalProgress {
    pub goal: Goal,
    /// Current value of the goal's share of its stores, in the goal currency.
    pub saved: Amount,
    /// Net money put into those stores, valued on the day it went in.
    pub contributed: Amount,
    /// `saved − contributed`: what rates added or took away.
    pub fx_gain: Amount,
    /// `saved / target × 100`.
    pub pct: Amount,
    /// Months from the start to the deadline, both included.
    pub months_total: Option<u32>,
    /// Months from this one to the deadline, both included.
    pub months_left: Option<u32>,
    /// `(target − saved) / months_left`.
    pub need_per_month: Option<Amount>,
    pub realistic: bool,
    pub issues: Vec<RealismIssue>,
    /// Date (`YYYY-MM-DD`) the target is reached at the recent pace.
    pub forecast: Option<String>,
    /// Whether the recent trend grows at all.
    pub accumulating: bool,
    /// What the linked calculator scenario plans to have by now, in the goal
    /// currency.
    pub planned_now: Option<Amount>,
    /// Operations that put money into the goal's wallets since its start.
    #[serde(default)]
    pub deposits: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct GoalCalendarMonth {
    /// `YYYY-MM`.
    pub month: String,
    /// Planned contributions of the goals active this month.
    pub need: Amount,
    /// Net money put into goal stores this month, without rate changes.
    pub saved: Amount,
    pub shortfall: Amount,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct GoalCalendar {
    pub year: i32,
    pub currency: String,
    pub months: Vec<GoalCalendarMonth>,
}

/// Months from `from` to `to`, both included; zero when `to` is earlier.
pub fn months_between(from: NaiveDate, to: NaiveDate) -> u32 {
    let span = (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32 + 1;
    span.max(0) as u32
}

pub fn hundred() -> Amount {
    Amount::new(rust_decimal::Decimal::from(100))
}

pub fn amount_of(count: u32) -> Amount {
    Amount::new(rust_decimal::Decimal::from(count))
}

/// Slope of a least-squares line through `(x, y)` points.
pub fn slope(points: &[(f64, f64)]) -> f64 {
    let count = points.len() as f64;
    if count < 2.0 {
        return 0.0;
    }
    let mean_x = points.iter().map(|(x_val, _)| x_val).sum::<f64>() / count;
    let mean_y = points.iter().map(|(_, y_val)| y_val).sum::<f64>() / count;
    let (num, den) = points
        .iter()
        .fold((0.0, 0.0), |(num, den), (x_val, y_val)| {
            (
                num + (x_val - mean_x) * (y_val - mean_y),
                den + (x_val - mean_x).powi(2),
            )
        });
    if den == 0.0 { 0.0 } else { num / den }
}

/// Goals by importance (more stars first), then by deadline.
pub fn goals(doc: &VaultDoc) -> Vec<Goal> {
    let mut out = doc.goals.clone();
    out.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| match (&left.deadline, &right.deadline) {
                (Some(one), Some(other)) => one.cmp(other),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
    });
    out
}

/// The goals the user has switched on, by importance.
pub fn active_goals(doc: &VaultDoc) -> Vec<Goal> {
    goals(doc).into_iter().filter(|goal| goal.active).collect()
}

pub fn check_goal(doc: &VaultDoc, draft: &GoalDraft) -> Result<()> {
    if draft.name.trim().is_empty() {
        return Err(Error::InvalidInput("name the goal".into()));
    }
    if !draft.target_amount.is_positive() {
        return Err(Error::InvalidInput("the target must be positive".into()));
    }
    if draft.currency.trim().is_empty() {
        return Err(Error::InvalidInput("choose a currency".into()));
    }
    if !(1..=5).contains(&draft.priority) {
        return Err(Error::InvalidInput("importance is 1–5".into()));
    }
    if draft
        .monthly_commitment
        .is_some_and(|value| value.is_negative())
    {
        return Err(Error::InvalidInput(
            "the monthly amount cannot be negative".into(),
        ));
    }
    let start = secs_or_now(&draft.starts_at);
    if let Some(deadline) = &draft.deadline
        && secs_or_now(deadline) < start
    {
        return Err(Error::InvalidInput(
            "the deadline is before the start".into(),
        ));
    }
    if draft.scope.store_ids.is_empty() {
        return Err(Error::InvalidInput(
            "choose the wallets that hold the savings".into(),
        ));
    }
    if let Some(rate) = draft.expected_return
        && (rate.is_negative() || rate > Amount::from(MAX_EXPECTED_RETURN))
    {
        return Err(Error::InvalidInput(
            "the expected return must be between 0 and 100% a year".into(),
        ));
    }
    if let Some(id) = &draft.scenario_id
        && find_calc_scenario(doc, id).is_none()
    {
        return Err(Error::Missing(format!("scenario {id}")));
    }
    match draft
        .scope
        .store_ids
        .iter()
        .find(|id| !doc.stores.iter().any(|store| &store.id == *id))
    {
        Some(id) => Err(Error::Missing(format!("store {id}"))),
        None => Ok(()),
    }
}

/// The goal a draft makes, named `id` and stamped `now`. Nothing is written.
pub fn plan_create_goal(doc: &VaultDoc, draft: GoalDraft, id: String, now: &str) -> Result<Goal> {
    check_goal(doc, &draft)?;
    Ok(Goal {
        id,
        name: draft.name.trim().to_string(),
        active: draft.active,
        target_amount: draft.target_amount,
        currency: draft.currency.trim().to_lowercase(),
        starts_at: draft.starts_at,
        deadline: draft.deadline,
        priority: draft.priority,
        monthly_commitment: draft.monthly_commitment,
        scope: draft.scope,
        created_at: now.to_string(),
        scenario_id: draft.scenario_id,
        expected_return: draft.expected_return,
    })
}

/// The goal `id` as a draft leaves it. Nothing is written.
pub fn plan_update_goal(doc: &VaultDoc, id: &str, draft: GoalDraft) -> Result<Goal> {
    check_goal(doc, &draft)?;
    let mut goal = doc
        .goals
        .iter()
        .find(|goal| goal.id == id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("goal {id}")))?;
    goal.name = draft.name.trim().to_string();
    goal.active = draft.active;
    goal.target_amount = draft.target_amount;
    goal.currency = draft.currency.trim().to_lowercase();
    goal.starts_at = draft.starts_at;
    goal.deadline = draft.deadline;
    goal.priority = draft.priority;
    goal.monthly_commitment = draft.monthly_commitment;
    goal.scope = draft.scope;
    goal.scenario_id = draft.scenario_id;
    goal.expected_return = draft.expected_return;
    Ok(goal)
}

/// What deleting goal `id` takes with it (New-Goal 5): the goal, its
/// envelopes and the workflows `savers_of` names for it. Nothing is written.
pub fn plan_delete_goal(
    doc: &VaultDoc,
    id: &str,
    savers_of: impl FnOnce(&Goal) -> Vec<String>,
) -> Result<GoalDeleted> {
    let goal = doc
        .goals
        .iter()
        .find(|goal| goal.id == id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("goal {id}")))?;
    let envelopes = doc
        .envelopes
        .iter()
        .filter(|envelope| envelope.goal_id.as_deref() == Some(id))
        .cloned()
        .collect();
    let workflow_ids = savers_of(&goal);
    Ok(GoalDeleted {
        goal,
        envelopes,
        workflow_ids,
    })
}

/// The workflows to switch back on when a deleted goal returns: those that
/// still exist. A workflow deleted since then has nothing to switch on.
pub fn plan_restore_goal(doc: &VaultDoc, deleted: &GoalDeleted) -> Result<Vec<String>> {
    let id = &deleted.goal.id;
    if doc.goals.iter().any(|goal| &goal.id == id) {
        return Err(Error::InvalidInput(format!("goal {id} already exists")));
    }
    Ok(deleted
        .workflow_ids
        .iter()
        .filter(|workflow| doc.workflows.iter().any(|each| &each.id == *workflow))
        .cloned()
        .collect())
}

// -- shares of shared stores ----------------------------------------------

/// Every store id a scope covers.
pub(crate) fn resolve_scope(books: &Books<'_>, scope: &StoreScope) -> HashSet<String> {
    crate::money::tree::resolve_scope(books.doc(), scope)
}

/// Current USD value of each store.
pub(crate) fn store_values_usd(books: &Books<'_>) -> HashMap<String, Amount> {
    let mut out: HashMap<String, Amount> = HashMap::new();
    for holding in &books.doc().holdings {
        if holding.asset.is_derivative() {
            continue;
        }
        *out.entry(holding.store_id.clone()).or_default() +=
            holding.quantity * books.price_usd(&holding.asset.key());
    }
    out
}

/// Planned monthly contribution in USD: target over the goal's months.
pub(crate) fn planned_usd(books: &Books<'_>, goal: &Goal, cal: &Calendar, now: i64) -> Amount {
    let target = books
        .rate_book()
        .convert(goal.target_amount, &goal.currency, "usd", now)
        .unwrap_or(goal.target_amount);
    match (cal.date_of_iso(&goal.starts_at), goal.deadline.as_deref()) {
        (Some(start), Some(deadline)) => {
            let months = cal
                .date_of_iso(deadline)
                .map_or(1, |end| months_between(start, end).max(1));
            target / amount_of(months)
        }
        _ => target,
    }
}

/// USD each active goal gets of its stores (plan §7.4): a store in one
/// goal is all that goal's; a store shared by several is split in
/// proportion to their planned monthly contribution. No goal gets more
/// than its target; the excess goes to the next goal by importance.
/// Returns `(allocated, scope value)` per goal id.
pub(crate) fn goal_allocations(
    books: &Books<'_>,
    cal: &Calendar,
    now: i64,
) -> HashMap<String, (Amount, Amount)> {
    let book = books.rate_book();
    let values = store_values_usd(books);
    let goals = active_goals(books.doc());
    let scopes: HashMap<String, HashSet<String>> = goals
        .iter()
        .map(|goal| (goal.id.clone(), resolve_scope(books, &goal.scope)))
        .collect();
    let weights: HashMap<String, Amount> = goals
        .iter()
        .map(|goal| (goal.id.clone(), planned_usd(books, goal, cal, now)))
        .collect();
    let mut stores: HashSet<&String> = HashSet::new();
    for scope in scopes.values() {
        stores.extend(scope.iter());
    }
    let mut allocated: HashMap<String, Amount> = HashMap::new();
    let mut scope_value: HashMap<String, Amount> = HashMap::new();
    for store_id in stores {
        let value = values.get(store_id).copied().unwrap_or(Amount::ZERO);
        let owners: Vec<&Goal> = goals
            .iter()
            .filter(|goal| scopes[&goal.id].contains(store_id))
            .collect();
        let total_weight = owners
            .iter()
            .fold(Amount::ZERO, |sum, goal| sum + weights[&goal.id]);
        for goal in &owners {
            *scope_value.entry(goal.id.clone()).or_default() += value;
            let share = if owners.len() == 1 || total_weight.is_zero() {
                value / amount_of(owners.len() as u32)
            } else {
                value * weights[&goal.id] / total_weight
            };
            *allocated.entry(goal.id.clone()).or_default() += share;
        }
    }
    // Cap at the target, then hand the excess down by importance.
    let mut excess = Amount::ZERO;
    let targets: HashMap<String, Amount> = goals
        .iter()
        .map(|goal| {
            let target = book
                .convert(goal.target_amount, &goal.currency, "usd", now)
                .unwrap_or(goal.target_amount);
            (goal.id.clone(), target)
        })
        .collect();
    for goal in &goals {
        let entry = allocated.entry(goal.id.clone()).or_default();
        if *entry > targets[&goal.id] {
            excess += *entry - targets[&goal.id];
            *entry = targets[&goal.id];
        }
    }
    for goal in &goals {
        if excess.is_zero() {
            break;
        }
        let entry = allocated.entry(goal.id.clone()).or_default();
        let room = targets[&goal.id] - *entry;
        if room.is_positive() {
            let given = room.min(excess);
            *entry += given;
            excess -= given;
        }
    }
    goals
        .iter()
        .map(|goal| {
            (
                goal.id.clone(),
                (
                    allocated.get(&goal.id).copied().unwrap_or_default(),
                    scope_value.get(&goal.id).copied().unwrap_or_default(),
                ),
            )
        })
        .collect()
}

/// Operations from `from` on with money coming into `scope`.
pub fn deposits(doc: &VaultDoc, scope: &HashSet<String>, from: i64) -> u32 {
    let count = doc
        .transactions
        .iter()
        .filter(|tx| secs_or_now(&tx.meta().timestamp) >= from)
        .filter(|tx| {
            tx.legs().iter().any(|leg| {
                leg.role == LegRole::Principal
                    && leg.amount.is_positive()
                    && scope.contains(&leg.store_id)
            })
        })
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The scenario's balance for the months passed since the goal started.
pub fn planned_now(
    books: &Books<'_>,
    goal: &Goal,
    start: NaiveDate,
    today: NaiveDate,
    now: i64,
) -> Option<Amount> {
    let scenario = find_calc_scenario(books.doc(), goal.scenario_id.as_deref()?)?;
    let elapsed = months_between(start, today).saturating_sub(1);
    let planned = planned_balance(&scenario.input, elapsed).ok()?;
    books
        .rate_book()
        .convert(planned, &scenario.input.currency, &goal.currency, now)
        .map(|value| value.round(2))
}
