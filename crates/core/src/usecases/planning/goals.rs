//! Scenarios of the goals area: each decides in `planning::goals`, then ends
//! in one `vault.transact`. Restoring a goal announces `GoalResumed` from
//! inside that transact, naming the workflows to start again; the reaction
//! runs after the commit (`usecases::react`). Deleting a goal first announces
//! `GoalPaused` in the same transact that removes it, naming the saving
//! workflows to stop.
//!
//! The progress queries sit here too: they gather what other areas know (the
//! envelopes, the month summaries, the history series) and leave the rules to
//! `planning::goals`.

use std::collections::HashSet;

use chrono::{Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, Calendar, first_of_month};
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::kernel::events::{DomainEvent, GoalPaused, GoalResumed};
use crate::model::planning::{Envelope, Goal};
use crate::model::transaction::LegRole;
use crate::model::vault::{VaultDoc, now_iso};
use crate::money::portfolio::history_series_filtered_in;
use crate::planning::budget::{budget_currency, manual_month, month_summary};
use crate::planning::envelopes::envelope_saved_for_goal;
use crate::planning::goals::{
    DAYS_PER_MONTH, FORECAST_DAYS, GoalCalendar, GoalCalendarMonth, GoalDeleted, GoalDraft,
    GoalProgress, RealismIssue, SURPLUS_MONTHS, active_goals, amount_of, deposits,
    goal_allocations, hundred, months_between, months_to_target, need_with_return,
    plan_create_goal, plan_delete_goal, plan_restore_goal, plan_update_goal, planned_now,
    resolve_scope, slope,
};
use crate::pricing::history::secs_or_now;
use crate::service::MIN_HISTORY_MONTHS;
use crate::service::UnlockedVault;
use crate::usecases::react::react;

/// Active workflows that move money into the goal's wallet (what
/// "Set aside 10% of every salary" makes), unless another goal saves
/// there too: a wallet's salary or a shared one keeps working.
pub fn goal_savers(doc: &VaultDoc, goal: &Goal) -> Vec<String> {
    let others: HashSet<&String> = doc
        .goals
        .iter()
        .filter(|other| other.id != goal.id)
        .flat_map(|other| &other.scope.store_ids)
        .collect();
    let stores: Vec<&String> = goal
        .scope
        .store_ids
        .iter()
        .filter(|store| !others.contains(store))
        .collect();
    doc.workflows
        .iter()
        .filter(|workflow| workflow.active && workflow.automation.is_none())
        .filter(|workflow| {
            crate::workflow::Graph::parse(&workflow.graph).is_ok_and(|graph| {
                graph.nodes.iter().any(|node| {
                    node.kind == "action.transaction"
                        && node
                            .params
                            .get("to")
                            .and_then(|to| to.as_str())
                            .is_some_and(|to| stores.iter().any(|store| store.as_str() == to))
                })
            })
        })
        .map(|workflow| workflow.id.clone())
        .collect()
}

pub fn create_goal(vault: &mut UnlockedVault, draft: GoalDraft) -> Result<Goal> {
    let goal = plan_create_goal(
        &vault.doc,
        draft,
        uuid::Uuid::new_v4().to_string(),
        &now_iso(),
    )?;
    vault.transact(|tx| {
        tx.collection::<Goal>().insert(goal.clone())?;
        Ok(goal)
    })
}

pub fn update_goal(vault: &mut UnlockedVault, id: &str, draft: GoalDraft) -> Result<Goal> {
    let goal = plan_update_goal(&vault.doc, id, draft)?;
    vault.transact(|tx| {
        tx.collection::<Goal>()
            .update(id, |slot| *slot = goal.clone())?;
        Ok(goal)
    })
}

/// Delete a goal (New-Goal 5): the money stays in its wallets, its
/// envelopes go back to "safe to spend", the workflows that only put
/// money aside into it stop. Its files go with the next open of the vault
/// unless [`restore_goal`] brings it back first.
pub fn delete_goal(vault: &mut UnlockedVault, id: &str, now: i64) -> Result<GoalDeleted> {
    let deleted = plan_delete_goal(&vault.doc, id, |goal| goal_savers(&vault.doc, goal))?;
    let paused = DomainEvent::from(GoalPaused {
        id: id.to_string(),
        workflow_ids: deleted.workflow_ids.clone(),
        at: now,
    });
    vault.transact(|tx| {
        tx.emit(paused.clone());
        tx.collection::<Goal>().remove(id)?;
        for envelope in &deleted.envelopes {
            tx.collection::<Envelope>().remove(&envelope.id)?;
        }
        Ok(())
    })?;
    match react(vault, vec![paused]).into_iter().next() {
        Some(failure) => Err(failure.error),
        None => Ok(deleted),
    }
}

/// Undo [`delete_goal`]: the goal, its envelopes and its workflows.
pub fn restore_goal(vault: &mut UnlockedVault, deleted: GoalDeleted, now: i64) -> Result<Goal> {
    let switch_on = plan_restore_goal(&vault.doc, &deleted)?;
    let GoalDeleted {
        goal, envelopes, ..
    } = deleted;
    let resumed = DomainEvent::from(GoalResumed {
        id: goal.id.clone(),
        workflow_ids: switch_on,
        at: now,
    });
    vault.transact(|tx| {
        tx.collection::<Goal>().insert(goal.clone())?;
        for envelope in envelopes {
            tx.collection::<Envelope>().insert(envelope)?;
        }
        tx.emit(resumed.clone());
        Ok(())
    })?;
    match react(vault, vec![resumed]).into_iter().next() {
        Some(failure) => Err(failure.error),
        None => Ok(goal),
    }
}

/// Net money put into `scope` within `[from, to)`, valued in `currency` on
/// the day it moved. Moves inside the scope cancel out; fees are losses,
/// not withdrawals, and do not count.
pub(crate) fn contributions(
    books: &Books<'_>,
    scope: &HashSet<String>,
    currency: &str,
    from: i64,
    to: i64,
) -> Amount {
    let book = books.rate_book();
    let mut total = Amount::ZERO;
    for tx in &books.doc().transactions {
        let secs = secs_or_now(&tx.meta().timestamp);
        if secs < from || secs >= to {
            continue;
        }
        for leg in tx.legs() {
            if leg.role != LegRole::Principal || !scope.contains(&leg.store_id) {
                continue;
            }
            let Some(value) = UnlockedVault::leg_value_in(book, &leg, currency, secs) else {
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
pub fn free_surplus_avg(books: &Books<'_>, now: i64, zone: &str) -> Option<Amount> {
    let cal = Calendar::new(zone);
    let mut month = first_of_month(cal.date_of(now));
    let mut found: Vec<Amount> = Vec::new();
    for _ in 0..24 {
        month = first_of_month(month - Duration::days(1));
        let summary = month_summary(books, &calendar::month_key(month), zone).ok()?;
        if summary.income.is_zero() && summary.fixed.is_zero() && summary.daily.is_zero() {
            continue;
        }
        found.push(summary.savings_fact);
        if found.len() == SURPLUS_MONTHS {
            break;
        }
    }
    // Short history: the user's own month, when given (plan B).
    if found.len() < MIN_HISTORY_MONTHS as usize
        && let Some(manual) = manual_month(books)
    {
        return Some(manual.income - manual.fixed - manual.daily);
    }
    if found.is_empty() {
        return None;
    }
    let sum = found.iter().fold(Amount::ZERO, |sum, value| sum + *value);
    Some(sum / amount_of(found.len() as u32))
}

/// Daily USD values of a scope over the last [`FORECAST_DAYS`] days.
fn daily_values(
    books: &Books<'_>,
    scope: &HashSet<String>,
    cal: &Calendar,
    now: i64,
) -> Vec<(f64, f64)> {
    let series = history_series_filtered_in(books.doc(), books.rate_book(), Some(scope), "usd");
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

pub fn goal_progress(books: &Books<'_>, now: i64, zone: &str) -> Vec<GoalProgress> {
    let cal = Calendar::new(zone);
    let book = books.rate_book();
    let today = cal.date_of(now);
    let allocations = goal_allocations(books, &cal, now);
    let mut out: Vec<GoalProgress> = active_goals(books.doc())
        .into_iter()
        .map(|goal| {
            let scope = resolve_scope(books, &goal.scope);
            let (alloc_usd, scope_usd) = allocations.get(&goal.id).copied().unwrap_or_default();
            let share = if scope_usd.is_zero() {
                Amount::ZERO
            } else {
                alloc_usd / scope_usd
            };
            // A goal backed by envelopes is exactly what they earmark.
            let saved = envelope_saved_for_goal(books, &goal.id, &goal.currency, now)
                .unwrap_or_else(|| {
                    book.convert(alloc_usd, "usd", &goal.currency, now)
                        .unwrap_or(Amount::ZERO)
                });
            let contributed =
                contributions(books, &scope, &goal.currency, i64::MIN, i64::MAX) * share;
            let start = cal.date_of_iso(&goal.starts_at).unwrap_or(today);
            let deadline = goal
                .deadline
                .as_deref()
                .and_then(|iso| cal.date_of_iso(iso));
            let months_total = deadline.map(|end| months_between(start, end).max(1));
            let months_left = deadline.map(|end| months_between(today.max(start), end).max(1));
            let left_to_save = (goal.target_amount - saved).max(Amount::ZERO);
            // Percent a year compounded monthly, as the calculator's Monthly.
            let monthly = goal
                .expected_return
                .map(|rate| rate / Amount::from(1200))
                .filter(|rate| rate.is_positive());
            let need_per_month = months_left.map(|months| match monthly {
                Some(rate) => need_with_return(saved, goal.target_amount, rate, months),
                None => left_to_save / amount_of(months),
            });
            let pct = (saved / goal.target_amount * hundred()).round(1);

            let points = daily_values(books, &scope, &cal, now);
            let per_day_usd = slope(&points) * share.to_f64();
            let accumulating = per_day_usd > 0.0;
            let forecast = if left_to_save.is_zero() {
                Some(today)
            } else if let Some(rate) = monthly {
                // The commitment, else the pace of the last months.
                let per_month = goal.monthly_commitment.unwrap_or_else(|| {
                    let usd = Amount::from_f64(per_day_usd.max(0.0) * DAYS_PER_MONTH);
                    book.convert(usd, "usd", &goal.currency, now)
                        .unwrap_or(Amount::ZERO)
                });
                months_to_target(saved, goal.target_amount, rate, per_month)
                    .and_then(|months| today.checked_add_months(chrono::Months::new(months)))
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
            let planned_now = planned_now(books, &goal, start, today, now);
            let deposits = deposits(books.doc(), &scope, secs_or_now(&goal.starts_at));
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
                planned_now,
                deposits,
            }
        })
        .collect();

    // All goals together against what is usually left over.
    let currency = budget_currency(books);
    if let Some(surplus) = free_surplus_avg(books, now, zone) {
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

pub fn goal_calendar(books: &Books<'_>, year: i32, now: i64, zone: &str) -> Result<GoalCalendar> {
    let cal = Calendar::new(zone);
    let book = books.rate_book();
    let currency = budget_currency(books);
    let first = NaiveDate::from_ymd_opt(year, 1, 1)
        .ok_or_else(|| Error::InvalidInput(format!("year {year}")))?;
    let goals = active_goals(books.doc());
    let mut scope: HashSet<String> = HashSet::new();
    for goal in &goals {
        scope.extend(resolve_scope(books, &goal.scope));
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
        let saved = contributions(books, &scope, &currency, from, to);
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
