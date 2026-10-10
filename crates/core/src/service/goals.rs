//! Goals (plan §7.4, D17, D23): how much is saved, how much the user put
//! in, how much to put aside a month, and whether that is realistic.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, Calendar, first_of_month};
use crate::error::{Error, Result};
use crate::model::planning::Goal;
use crate::model::store::StoreScope;
use crate::model::transaction::LegRole;
use crate::model::vault::now_iso;
use crate::pricing::history::secs_or_now;

use super::UnlockedVault;

/// How many non-empty past months the free surplus averages (plan §7.4).
pub const SURPLUS_MONTHS: usize = 6;
/// Days of history the pace forecast fits a trend to.
pub const FORECAST_DAYS: i64 = 90;

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
}

/// Why a goal is marked «Пересмотреть».
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
fn months_between(from: NaiveDate, to: NaiveDate) -> u32 {
    let span = (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32 + 1;
    span.max(0) as u32
}

fn hundred() -> Amount {
    Amount::new(rust_decimal::Decimal::from(100))
}

fn amount_of(count: u32) -> Amount {
    Amount::new(rust_decimal::Decimal::from(count))
}

/// Slope of a least-squares line through `(x, y)` points.
fn slope(points: &[(f64, f64)]) -> f64 {
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

impl UnlockedVault {
    // -- CRUD ---------------------------------------------------------------

    /// Goals by importance (more stars first), then by deadline.
    pub fn goals(&self) -> Vec<Goal> {
        let mut out = self.doc.goals.clone();
        out.sort_by(|left, right| {
            right.priority.cmp(&left.priority).then_with(|| {
                match (&left.deadline, &right.deadline) {
                    (Some(one), Some(other)) => one.cmp(other),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            })
        });
        out
    }

    fn check_goal(&self, draft: &GoalDraft) -> Result<()> {
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
        match draft
            .scope
            .store_ids
            .iter()
            .find(|id| self.find_store(id).is_none())
        {
            Some(id) => Err(Error::Missing(format!("store {id}"))),
            None => Ok(()),
        }
    }

    pub fn create_goal(&mut self, draft: GoalDraft) -> Result<Goal> {
        self.check_goal(&draft)?;
        let goal = Goal {
            id: uuid::Uuid::new_v4().to_string(),
            name: draft.name.trim().to_string(),
            active: draft.active,
            target_amount: draft.target_amount,
            currency: draft.currency.trim().to_lowercase(),
            starts_at: draft.starts_at,
            deadline: draft.deadline,
            priority: draft.priority,
            monthly_commitment: draft.monthly_commitment,
            scope: draft.scope,
            created_at: now_iso(),
        };
        self.doc.goals.push(goal.clone());
        self.persist()?;
        Ok(goal)
    }

    pub fn update_goal(&mut self, id: &str, draft: GoalDraft) -> Result<Goal> {
        self.check_goal(&draft)?;
        let goal = self
            .doc
            .goals
            .iter_mut()
            .find(|goal| goal.id == id)
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
        let out = goal.clone();
        self.persist()?;
        Ok(out)
    }

    pub fn delete_goal(&mut self, id: &str) -> Result<()> {
        let before = self.doc.goals.len();
        self.doc.goals.retain(|goal| goal.id != id);
        if self.doc.goals.len() == before {
            return Err(Error::Missing(format!("goal {id}")));
        }
        self.persist()
    }

    // -- shares of shared stores ----------------------------------------------

    /// Current USD value of each store.
    fn store_values_usd(&self) -> HashMap<String, Amount> {
        let mut out: HashMap<String, Amount> = HashMap::new();
        for holding in &self.doc.holdings {
            if holding.asset.is_derivative() {
                continue;
            }
            *out.entry(holding.store_id.clone()).or_default() +=
                holding.quantity * self.price_usd_for(&holding.asset.key());
        }
        out
    }

    /// Planned monthly contribution in USD: target over the goal's months.
    fn planned_usd(&self, goal: &Goal, cal: &Calendar, now: i64) -> Amount {
        let book = self.rate_book();
        let target = book
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
    fn goal_allocations(&self, cal: &Calendar, now: i64) -> HashMap<String, (Amount, Amount)> {
        let book = self.rate_book();
        let values = self.store_values_usd();
        let goals: Vec<Goal> = self
            .goals()
            .into_iter()
            .filter(|goal| goal.active)
            .collect();
        let scopes: HashMap<String, HashSet<String>> = goals
            .iter()
            .map(|goal| (goal.id.clone(), self.resolve_scope(&goal.scope)))
            .collect();
        let weights: HashMap<String, Amount> = goals
            .iter()
            .map(|goal| (goal.id.clone(), self.planned_usd(goal, cal, now)))
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

    /// Net money put into `scope` within `[from, to)`, valued in `currency` on
    /// the day it moved. Moves inside the scope cancel out; fees are losses,
    /// not withdrawals, and do not count.
    fn contributions(&self, scope: &HashSet<String>, currency: &str, from: i64, to: i64) -> Amount {
        let book = self.rate_book();
        let mut total = Amount::ZERO;
        for tx in &self.doc.transactions {
            let secs = secs_or_now(&tx.meta().timestamp);
            if secs < from || secs >= to {
                continue;
            }
            for leg in tx.legs() {
                if leg.role != LegRole::Principal || !scope.contains(&leg.store_id) {
                    continue;
                }
                let Some(value) = Self::leg_value_in(&book, &leg, currency, secs) else {
                    continue;
                };
                if leg.amount.is_negative() {
                    total -= value;
                } else {
                    total += value;
                }
            }
        }
        total
    }

    /// Average of `income − fixed − daily` over the last non-empty months
    /// before the current one, in the budget currency; `None` without any.
    pub fn free_surplus_avg(&self, now: i64, zone: &str) -> Option<Amount> {
        let cal = Calendar::new(zone);
        let mut month = first_of_month(cal.date_of(now));
        let mut found: Vec<Amount> = Vec::new();
        for _ in 0..24 {
            month = first_of_month(month - Duration::days(1));
            let summary = self.month_summary(&calendar::month_key(month), zone).ok()?;
            if summary.income.is_zero() && summary.fixed.is_zero() && summary.daily.is_zero() {
                continue;
            }
            found.push(summary.savings_fact);
            if found.len() == SURPLUS_MONTHS {
                break;
            }
        }
        if found.is_empty() {
            return None;
        }
        let sum = found.iter().fold(Amount::ZERO, |sum, value| sum + *value);
        Some(sum / amount_of(found.len() as u32))
    }

    // -- progress -------------------------------------------------------------

    /// Daily USD values of a scope over the last [`FORECAST_DAYS`] days.
    fn daily_values(&self, scope: &HashSet<String>, cal: &Calendar, now: i64) -> Vec<(f64, f64)> {
        let series = self.history_series_filtered(Some(scope), "usd");
        let today = cal.date_of(now);
        (0..=FORECAST_DAYS)
            .map(|offset| {
                let day = today - Duration::days(FORECAST_DAYS - offset);
                let end_ms = cal.start_of(day + Duration::days(1)) * 1000;
                let value = series
                    .iter()
                    .rev()
                    .find(|point| point.ts_ms < end_ms)
                    .map_or(0.0, |point| point.value_usd.to_f64());
                (offset as f64, value)
            })
            .collect()
    }

    /// Progress of every active goal (plan §7.4, D23).
    pub fn goal_progress(&self, now: i64, zone: &str) -> Vec<GoalProgress> {
        let cal = Calendar::new(zone);
        let book = self.rate_book();
        let today = cal.date_of(now);
        let allocations = self.goal_allocations(&cal, now);
        let mut out: Vec<GoalProgress> = self
            .goals()
            .into_iter()
            .filter(|goal| goal.active)
            .map(|goal| {
                let scope = self.resolve_scope(&goal.scope);
                let (alloc_usd, scope_usd) = allocations.get(&goal.id).copied().unwrap_or_default();
                let share = if scope_usd.is_zero() {
                    Amount::ZERO
                } else {
                    alloc_usd / scope_usd
                };
                let saved = book
                    .convert(alloc_usd, "usd", &goal.currency, now)
                    .unwrap_or(Amount::ZERO);
                let contributed =
                    self.contributions(&scope, &goal.currency, i64::MIN, i64::MAX) * share;
                let start = cal.date_of_iso(&goal.starts_at).unwrap_or(today);
                let deadline = goal
                    .deadline
                    .as_deref()
                    .and_then(|iso| cal.date_of_iso(iso));
                let months_total = deadline.map(|end| months_between(start, end).max(1));
                let months_left = deadline.map(|end| months_between(today.max(start), end).max(1));
                let left_to_save = (goal.target_amount - saved).max(Amount::ZERO);
                let need_per_month = months_left.map(|months| left_to_save / amount_of(months));
                let pct = (saved / goal.target_amount * hundred()).round(1);

                let points = self.daily_values(&scope, &cal, now);
                let per_day_usd = slope(&points) * share.to_f64();
                let accumulating = per_day_usd > 0.0;
                let forecast = if left_to_save.is_zero() {
                    Some(today)
                } else if accumulating {
                    let left_usd = book
                        .convert(left_to_save, &goal.currency, "usd", now)
                        .map_or(0.0, |value| value.to_f64());
                    let days = (left_usd / per_day_usd).ceil();
                    (days < 365.0 * 200.0).then(|| today + Duration::days(days as i64))
                } else {
                    None
                };

                let mut issues = Vec::new();
                if let (Some(need), Some(commitment)) = (need_per_month, goal.monthly_commitment)
                    && need > commitment
                {
                    issues.push(RealismIssue::OverCommitment { need, commitment });
                }
                GoalProgress {
                    goal,
                    saved,
                    contributed,
                    fx_gain: saved - contributed,
                    pct,
                    months_total,
                    months_left,
                    need_per_month,
                    realistic: true,
                    issues,
                    forecast: forecast.map(|date| date.format("%Y-%m-%d").to_string()),
                    accumulating,
                }
            })
            .collect();

        // All goals together against what is usually left over.
        let currency = self.budget_currency();
        if let Some(surplus) = self.free_surplus_avg(now, zone) {
            let need_total = out
                .iter()
                .filter_map(|progress| {
                    let need = progress.need_per_month?;
                    book.convert(need, &progress.goal.currency, &currency, now)
                })
                .fold(Amount::ZERO, |sum, value| sum + value);
            if need_total > surplus {
                for progress in out.iter_mut().filter(|item| item.need_per_month.is_some()) {
                    progress.issues.push(RealismIssue::OverSurplus {
                        need_total,
                        surplus,
                        currency: currency.clone(),
                    });
                }
            }
        }
        for progress in &mut out {
            progress.realistic = progress.issues.is_empty();
        }
        out
    }

    /// «Нужно отложить / отложено» by month of a year (plan §7.4), in the
    /// budget currency.
    pub fn goal_calendar(&self, year: i32, now: i64, zone: &str) -> Result<GoalCalendar> {
        let cal = Calendar::new(zone);
        let book = self.rate_book();
        let currency = self.budget_currency();
        let first = NaiveDate::from_ymd_opt(year, 1, 1)
            .ok_or_else(|| Error::InvalidInput(format!("year {year}")))?;
        let goals: Vec<Goal> = self
            .goals()
            .into_iter()
            .filter(|goal| goal.active)
            .collect();
        let mut scope: HashSet<String> = HashSet::new();
        for goal in &goals {
            scope.extend(self.resolve_scope(&goal.scope));
        }
        let mut months = Vec::with_capacity(12);
        let mut month = first;
        for _ in 0..12 {
            let mut need = Amount::ZERO;
            for goal in &goals {
                let (Some(start), Some(deadline)) = (
                    cal.date_of_iso(&goal.starts_at),
                    goal.deadline
                        .as_deref()
                        .and_then(|iso| cal.date_of_iso(iso)),
                ) else {
                    continue;
                };
                if month < first_of_month(start) || month > first_of_month(deadline) {
                    continue;
                }
                let months_total = months_between(start, deadline).max(1);
                need += book
                    .convert(goal.target_amount, &goal.currency, &currency, now)
                    .unwrap_or(Amount::ZERO)
                    / amount_of(months_total);
            }
            let (from, to) = cal.month_bounds(month);
            let saved = self.contributions(&scope, &currency, from, to);
            months.push(GoalCalendarMonth {
                month: calendar::month_key(month),
                need,
                saved,
                shortfall: (need - saved).max(Amount::ZERO),
                done: saved >= need,
            });
            month = calendar::next_month(month);
        }
        Ok(GoalCalendar {
            year,
            currency,
            months,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::transaction::AccountRef;
    use crate::model::vault::PricePoint;
    use crate::pricing::history::unix_secs;
    use crate::service::test_support::*;

    fn secs(iso: &str) -> i64 {
        unix_secs(iso).unwrap()
    }

    fn goal(name: &str, target: &str, stores: &[&str], priority: u8) -> GoalDraft {
        GoalDraft {
            name: name.into(),
            active: true,
            target_amount: amount(target),
            currency: "usd".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: Some("2026-12-31T00:00:00Z".into()),
            priority,
            monthly_commitment: None,
            scope: StoreScope {
                store_ids: stores.iter().map(|id| id.to_string()).collect(),
                include_children: true,
            },
        }
    }

    fn deposit(vault: &mut UnlockedVault, store: &str, sum: &str, ts: &str) {
        let mut tx = income(store, sum, "salary");
        at(&mut tx, ts);
        vault.add_transaction(tx).unwrap();
    }

    fn setup_savings() -> (tempfile::TempDir, UnlockedVault, String) {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
        (dir, vault, savings)
    }

    #[test]
    fn two_goals_on_one_account_split_it_and_never_exceed_their_target() {
        let (_dir, mut vault, savings) = setup_savings();
        deposit(&mut vault, &savings, "3000", "2026-02-01T00:00:00Z");
        // Same months: weights 1200 : 2400 → shares 1 : 2.
        vault
            .create_goal(goal("Laptop", "1200", &[&savings], 3))
            .unwrap();
        vault
            .create_goal(goal("Trip", "2400", &[&savings], 3))
            .unwrap();
        let progress = vault.goal_progress(secs("2026-03-01T00:00:00Z"), "UTC");
        let by_name = |name: &str| progress.iter().find(|item| item.goal.name == name).unwrap();
        assert_eq!(by_name("Laptop").saved, amount("1000"));
        assert_eq!(by_name("Trip").saved, amount("2000"));

        // 4000 is more than both need: each is capped at its target.
        deposit(&mut vault, &savings, "1000", "2026-02-02T00:00:00Z");
        deposit(&mut vault, &savings, "2000", "2026-02-03T00:00:00Z");
        let capped = vault.goal_progress(secs("2026-03-01T00:00:00Z"), "UTC");
        let laptop = capped
            .iter()
            .find(|item| item.goal.name == "Laptop")
            .unwrap();
        let trip = capped.iter().find(|item| item.goal.name == "Trip").unwrap();
        assert_eq!(laptop.saved, amount("1200"));
        assert_eq!(trip.saved, amount("2400"));
        assert_eq!(laptop.pct, amount("100"));
    }

    #[test]
    fn need_per_month_and_both_realism_reasons() {
        let (_dir, mut vault, savings) = setup_savings();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        // A month with 1000 income and 900 spent: 100 left over.
        deposit(&mut vault, &card, "1000", "2026-01-05T00:00:00Z");
        let mut spend = payment(&card, "900", "food");
        at(&mut spend, "2026-01-06T00:00:00Z");
        vault.add_transaction(spend).unwrap();

        let mut draft = goal("Car", "12000", &[&savings], 5);
        draft.monthly_commitment = Some(amount("500"));
        vault.create_goal(draft).unwrap();
        // February: 11 months left (Feb–Dec), nothing saved.
        let progress = vault.goal_progress(secs("2026-02-10T00:00:00Z"), "UTC");
        let car = &progress[0];
        assert_eq!(car.months_total, Some(12));
        assert_eq!(car.months_left, Some(11));
        assert_eq!(car.need_per_month.unwrap().round(2), amount("1090.91"));
        assert!(!car.realistic);
        assert_eq!(car.issues.len(), 2, "{:?}", car.issues);
        assert!(matches!(car.issues[0], RealismIssue::OverCommitment { .. }));
        let RealismIssue::OverSurplus { surplus, .. } = &car.issues[1] else {
            panic!()
        };
        assert_eq!(*surplus, amount("100"));
    }

    #[test]
    fn crypto_goal_contributed_stays_while_saved_follows_the_rate() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        let wallet = store(&mut vault, "Cold", StoreKind::ColdWallet, None);
        let mut buy = income_of(&wallet, btc(), "1", "50000", "bought");
        at(&mut buy, "2026-01-10T00:00:00Z");
        vault.add_transaction(buy).unwrap();
        vault.doc.price_cache.push(PricePoint {
            asset_key: btc().key(),
            value_usd: amount("80000"),
            ts: "2026-02-01T00:00:00Z".into(),
            source: "test".into(),
        });
        vault
            .create_goal(goal("House", "100000", &[&wallet], 3))
            .unwrap();
        let progress = vault.goal_progress(secs("2026-02-10T00:00:00Z"), "UTC");
        assert_eq!(progress[0].contributed, amount("50000"));
        assert_eq!(progress[0].saved, amount("80000"));
        assert_eq!(progress[0].fx_gain, amount("30000"));
    }

    #[test]
    fn a_flat_or_falling_trend_has_no_forecast() {
        let (_dir, mut vault, savings) = setup_savings();
        deposit(&mut vault, &savings, "500", "2025-06-01T00:00:00Z");
        vault
            .create_goal(goal("Sofa", "1000", &[&savings], 2))
            .unwrap();
        let flat = vault.goal_progress(secs("2026-02-10T00:00:00Z"), "UTC");
        assert!(!flat[0].accumulating);
        assert_eq!(flat[0].forecast, None);

        // Steady deposits over the last 90 days: a forecast appears.
        for day in ["2026-01-01", "2026-01-15", "2026-02-01"] {
            deposit(&mut vault, &savings, "100", &format!("{day}T00:00:00Z"));
        }
        let growing = vault.goal_progress(secs("2026-02-10T00:00:00Z"), "UTC");
        assert!(growing[0].accumulating);
        assert!(growing[0].forecast.is_some());
    }

    #[test]
    fn calendar_compares_planned_and_put_aside_by_month() {
        let (_dir, mut vault, savings) = setup_savings();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        vault
            .create_goal(goal("Laptop", "1200", &[&savings], 3))
            .unwrap();
        deposit(&mut vault, &card, "5000", "2026-01-01T00:00:00Z");
        let mut put_aside = transfer_between(
            AccountRef::Store(card.clone()),
            AccountRef::Store(savings.clone()),
            usd(),
            "150",
        );
        at(&mut put_aside, "2026-01-20T00:00:00Z");
        vault.add_transaction(put_aside).unwrap();
        let mut little = transfer_between(
            AccountRef::Store(card.clone()),
            AccountRef::Store(savings.clone()),
            usd(),
            "40",
        );
        at(&mut little, "2026-02-20T00:00:00Z");
        vault.add_transaction(little).unwrap();

        let calendar = vault
            .goal_calendar(2026, secs("2026-03-01T00:00:00Z"), "UTC")
            .unwrap();
        let jan = &calendar.months[0];
        assert_eq!(
            (jan.need, jan.saved, jan.done),
            (amount("100"), amount("150"), true)
        );
        let feb = &calendar.months[1];
        assert_eq!(feb.shortfall, amount("60"));
        assert!(!feb.done);
    }
}
