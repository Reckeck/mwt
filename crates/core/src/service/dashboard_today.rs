//! The dashboard's «Сегодня» strip and the calculator-scenario widget
//! (plan H4–H5): one request each, built from what the other widgets show.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use super::UnlockedVault;
use super::agenda::UpcomingKind;
use crate::Amount;
use crate::calc::invest;
use crate::calendar::Calendar;
use crate::error::{Error, Result};
use crate::ipc_contract::Comparison;
use crate::model::planning::HealthColor;
use crate::model::store::StoreScope;
use crate::model::vault::DashPeriod;

/// The strip lists at most this many things to do.
pub const TODAY_TODOS: usize = 3;
/// How far ahead the strip looks for payments and expiring things.
const TODAY_HORIZON_DAYS: u32 = 14;

/// What the strip asks the user to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum TodayTodoKind {
    /// A payment the accounts will not cover by its date.
    Shortfall,
    /// An automation run that failed and waits for «Повторить / Пропустить».
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
}

/// One thing to do, with what it is about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct DashboardToday {
    pub period: DashPeriod,
    /// Base currency of `net_worth`.
    pub currency: String,
    /// The capital now against the period's first day.
    pub net_worth: Comparison,
    /// The checkup's light; `None` when the checkup cannot be built.
    pub health: Option<HealthColor>,
    /// «Можно тратить сегодня», in `budget_currency`.
    pub left_today: Amount,
    pub budget_currency: String,
    /// Up to three, the earliest first.
    pub todos: Vec<TodayTodo>,
}

/// A calculator scenario against what really happened (plan H4, decision 12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
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
fn months_since(start: &str, today: NaiveDate) -> u32 {
    let Some(first) = NaiveDate::parse_from_str(&format!("{start}-01"), "%Y-%m-%d").ok() else {
        return 0;
    };
    let span = (today.year() - first.year()) * 12 + today.month() as i32 - first.month() as i32;
    span.max(0) as u32
}

impl UnlockedVault {
    /// Value of the wallets of `scope` in `currency`.
    fn scope_value_in(&self, scope: &StoreScope, currency: &str, now: i64) -> Option<Amount> {
        let values = self.store_values_usd();
        let usd = self
            .resolve_scope(scope)
            .iter()
            .filter_map(|id| values.get(id))
            .fold(Amount::ZERO, |sum, value| sum + *value);
        self.rate_book().convert(usd, "usd", currency, now)
    }

    /// Plan against fact for one saved scenario: the fact is its «Из данных»
    /// wallets, or else the wallets of the goal that follows it.
    pub fn scenario_status(&self, id: &str, now: i64, zone: &str) -> Result<ScenarioStatus> {
        let scenario = self
            .find_calc_scenario(id)
            .ok_or_else(|| Error::Missing(format!("scenario {id}")))?;
        let input = &scenario.input;
        let today = Calendar::new(zone).date_of(now);
        let elapsed = months_since(&input.start, today).min(input.term_months);
        let result = invest::simulate(input)?;
        let planned_now = super::planned_balance(input, elapsed)?;
        let goal = self
            .doc
            .goals
            .iter()
            .find(|goal| goal.active && goal.scenario_id.as_deref() == Some(id));
        let scope = scenario
            .source
            .as_ref()
            .map(|source| &source.scope)
            .or(goal.map(|goal| &goal.scope));
        let fact = scope
            .and_then(|scope| self.scope_value_in(scope, &input.currency, now))
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

    /// Things waiting for the user, the earliest first, at most [`TODAY_TODOS`].
    fn today_todos(&self, now: i64, zone: &str) -> Vec<TodayTodo> {
        let today = Calendar::new(zone).date_of(now);
        let horizon = today + Duration::days(i64::from(TODAY_HORIZON_DAYS));
        let within = |date: &str| {
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .is_ok_and(|day| day >= today && day <= horizon)
        };
        let mut todos = Vec::new();
        for item in self.upcoming(TODAY_HORIZON_DAYS, now, zone) {
            let kind = match item.kind {
                UpcomingKind::RejectedRun => TodayTodoKind::RejectedRun,
                UpcomingKind::UnconfirmedRun => TodayTodoKind::UnconfirmedRun,
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
        let names: BTreeMap<&str, &str> = self
            .doc
            .workflows
            .iter()
            .map(|flow| (flow.id.as_str(), flow.name.as_str()))
            .collect();
        for queued in &self.doc.workflow_queue {
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
        for fill in &self.doc.pending_imports {
            *pending.entry(fill.store_id.as_str()).or_default() += 1;
        }
        for (store_id, count) in pending {
            todos.push(TodayTodo {
                kind: TodayTodoKind::PendingImport,
                date: None,
                title: self
                    .find_store(store_id)
                    .map_or_else(String::new, |store| store.name.clone()),
                source_id: store_id.to_owned(),
                amount: Some(Amount::from(count)),
            });
        }
        for store in self.doc.stores.iter().filter(|store| !store.archived) {
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
        // Undated things are due already; the sort is stable, so the order
        // above breaks ties.
        todos.sort_by(|left, right| left.date.cmp(&right.date));
        todos.truncate(TODAY_TODOS);
        todos
    }

    /// Everything «Сегодня» shows, in one request (plan H5).
    pub fn dashboard_today(
        &self,
        period: DashPeriod,
        now: i64,
        zone: &str,
        manual: &BTreeMap<String, Amount>,
    ) -> DashboardToday {
        let trend = self.net_worth_trend(period, now, zone);
        let budget = self.daily_budget(now, zone);
        DashboardToday {
            period,
            currency: trend.currency,
            net_worth: trend.change,
            health: self
                .capital_health(now, zone, manual)
                .ok()
                .map(|report| report.color),
            left_today: budget.left_today,
            budget_currency: budget.currency,
            todos: self.today_todos(now, zone),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::invest::{CapFreq, ContributionStage, Freq, InvestInput, TaxRule, Timing};
    use crate::model::planning::ScenarioSource;
    use crate::model::store::{LoyaltyInfo, StoreKind};
    use crate::pricing::history::unix_secs;
    use crate::service::CalcScenarioDraft;
    use crate::service::test_support::*;

    #[test]
    fn months_since_counts_whole_months_from_the_start() {
        let day = |raw: &str| NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap();
        assert_eq!(months_since("2026-01", day("2026-04-10")), 3);
        assert_eq!(months_since("2026-05", day("2026-04-10")), 0);
        assert_eq!(months_since("bad", day("2026-04-10")), 0);
    }

    #[test]
    fn a_scenario_shows_its_plan_to_date_against_its_wallets() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        vault.add_transaction(income(&card, "1500", "pay")).unwrap();
        let scenario = vault
            .save_calc_scenario(CalcScenarioDraft {
                id: None,
                name: "Plan".into(),
                input: InvestInput {
                    currency: "usd".into(),
                    start: "2026-01".into(),
                    initial: amount("1000"),
                    rate_pct: Amount::ZERO,
                    term_months: 12,
                    capitalization: CapFreq::Monthly,
                    stages: vec![ContributionStage {
                        from_month: 1,
                        amount: amount("100"),
                        freq: Freq::Monthly,
                        timing: Timing::End,
                    }],
                    indexation_pct: Amount::ZERO,
                    inflation_pct: Amount::ZERO,
                    tax: TaxRule::default(),
                    withdrawal: None,
                },
                source: Some(ScenarioSource {
                    scope: StoreScope {
                        store_ids: vec![card],
                        include_children: false,
                    },
                    income_months: 3,
                    income_pct: amount("10"),
                }),
                prefilled_at: None,
            })
            .unwrap();
        let now = unix_secs("2026-04-10T10:00:00Z").unwrap();
        let status = vault.scenario_status(&scenario.id, now, "UTC").unwrap();
        assert_eq!(status.elapsed_months, 3);
        assert_eq!(status.planned_now, amount("1300"));
        assert_eq!(status.final_nominal, amount("2200"));
        assert_eq!(status.fact, Some(amount("1500")));
        assert!(vault.scenario_status("missing", now, "UTC").is_err());
    }

    #[test]
    fn the_strip_keeps_three_todos_due_first() {
        let (_dir, mut vault) = empty_vault();
        let now = unix_secs("2026-04-10T10:00:00Z").unwrap();
        for (name, date) in [
            ("Late", "2026-04-20"),
            ("Soon", "2026-04-12"),
            ("Far", "2026-06-01"),
            ("Mid", "2026-04-15"),
        ] {
            let id = store(&mut vault, name, StoreKind::Loyalty, None);
            let found = vault
                .doc
                .stores
                .iter_mut()
                .find(|item| item.id == id)
                .unwrap();
            found.loyalty = Some(LoyaltyInfo {
                points_symbol: name.to_uppercase(),
                point_value: amount("1"),
                currency: "rub".into(),
                expires_at: Some(date.into()),
                expiring_points: Some(amount("10")),
                login: None,
                source_store_ids: Vec::new(),
            });
        }
        let todos = vault.today_todos(now, "UTC");
        let titles: Vec<_> = todos.iter().map(|todo| todo.title.as_str()).collect();
        assert_eq!(titles, ["Soon", "Mid", "Late"]);
        assert!(
            todos
                .iter()
                .all(|todo| todo.kind == TodayTodoKind::PointsExpire)
        );

        let today = vault.dashboard_today(DashPeriod::Month, now, "UTC", &BTreeMap::new());
        assert_eq!(today.todos.len(), TODAY_TODOS);
    }
}
