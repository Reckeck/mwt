//! Scenarios and queries of the budget area. Each scenario decides in
//! `planning::budget`, then ends in one `vault.transact`.
//!
//! The queries take the `Books` of one request, so a request builds its rates
//! once; only the entry points read the vault.

use std::collections::HashSet;

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::error::Result;
use crate::model::planning::{Budget, BudgetPeriod, BudgetTarget};
use crate::money::books::Books;
use crate::money::worth::FlowClass;
use crate::planning::automation_plan::automation_runs;
use crate::planning::budget::{
    BudgetDraft, BudgetStatus, MonthSummary, budget_currency, month_summary_by, not_executed,
    period_dates, plan_create_budget, plan_delete_budget, plan_update_budget, used_of,
};
use crate::planning::flows::{Basis, Flow, flows_between, flows_for_basis};
use crate::planning::goals::resolve_scope;
use crate::service::Session;

// -- CRUD -------------------------------------------------------------------

pub fn create_budget(vault: &mut Session, draft: BudgetDraft) -> Result<Budget> {
    let budget = plan_create_budget(&vault.doc, draft, uuid::Uuid::new_v4().to_string())?;
    vault.transact(|tx| {
        tx.collection::<Budget>().insert(budget.clone())?;
        Ok(budget)
    })
}

pub fn update_budget(vault: &mut Session, id: &str, draft: BudgetDraft) -> Result<Budget> {
    let budget = plan_update_budget(&vault.doc, id, draft)?;
    vault.transact(|tx| {
        tx.collection::<Budget>()
            .update(id, |slot| *slot = budget.clone())?;
        Ok(budget)
    })
}

/// Delete a limit; its notices go with it, each with a tombstone, or another
/// device would send them back.
pub fn delete_budget(vault: &mut Session, id: &str) -> Result<()> {
    let gone = plan_delete_budget(&vault.doc, id)?;
    vault.transact(|tx| {
        tx.collection::<Budget>().remove(id)?;
        tx.doc_mut()
            .notification_log
            .retain(|record| !gone.contains(&record.key));
        for key in gone {
            tx.forget_log_record(format!("notification_log:{key}"));
        }
        Ok(())
    })
}

// -- flows ------------------------------------------------------------------

/// Operations the budget could not value in its currency: a side had no
/// rate (Currency-1 · 7, "Show").
pub(crate) fn unpriced_tx_ids(books: &Books<'_>) -> HashSet<String> {
    let currency = budget_currency(books);
    flows_between(books, &Calendar::new("UTC"), &currency, i64::MIN, i64::MAX)
        .into_iter()
        .filter(|flow| !flow.priced())
        .map(|flow| flow.tx_id)
        .collect()
}

// -- limits -----------------------------------------------------------------

/// Every active limit with what its current period used (plan §7.3):
/// mandatory and everyday spending of the budget stores, by category, by
/// wallet or in total, in the limit's own currency at each day's rate.
///
/// `Month` limits read by the default basis (`period`, op-period R4).
pub fn budget_status(books: &Books<'_>, now: i64, zone: &str) -> Vec<BudgetStatus> {
    budget_status_by(books, now, zone, None)
}

/// `budget_status` read by `basis`; `None` means `period`. Only `Month`
/// limits follow it: `Day` and `Week` limits stay on the money date (R3).
pub fn budget_status_by(
    books: &Books<'_>,
    now: i64,
    zone: &str,
    basis: Option<Basis>,
) -> Vec<BudgetStatus> {
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    books
        .doc()
        .budgets
        .iter()
        .filter(|budget| budget.active)
        .map(|budget| {
            let (start, end) = period_dates(budget.period, today);
            let scope: Option<HashSet<String>> = match &budget.target {
                BudgetTarget::Store(scope) => Some(resolve_scope(books, scope)),
                _ => None,
            };
            let basis = match budget.period {
                BudgetPeriod::Month => basis.unwrap_or(Basis::Period),
                _ => Basis::MoneyDate,
            };
            let counted: Vec<Flow> = flows_for_basis(
                books,
                &cal,
                &budget.currency,
                cal.start_of(start),
                cal.start_of(end),
                basis,
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
            .collect();
            let spent = counted
                .iter()
                .fold(Amount::ZERO, |sum, flow| sum + flow.value);
            // A flow without a rate adds nothing to `spent`: say so, do not pass it for zero (C2-8).
            let not_valued = counted.iter().any(|flow| !flow.priced());
            BudgetStatus {
                budget: budget.clone(),
                spent,
                remaining: budget.limit - spent,
                used: used_of(spent, budget.limit),
                period_start: calendar::iso(cal.start_of(start)),
                period_end: calendar::iso(cal.start_of(end)),
                not_valued,
            }
        })
        .collect()
}

// -- month totals -----------------------------------------------------------

/// Income, mandatory spending, everyday spending and savings of a calendar
/// month (`YYYY-MM`) in the budget currency (plan §7.3): the books' figures
/// plus the automation runs that did not execute.
pub fn month_summary(books: &Books<'_>, month: &str, zone: &str) -> Result<MonthSummary> {
    month_summary_with_basis(books, month, zone, None)
}

/// `month_summary` read by `basis`; `None` means `period` (R4).
pub fn month_summary_with_basis(
    books: &Books<'_>,
    month: &str,
    zone: &str,
    basis: Option<Basis>,
) -> Result<MonthSummary> {
    let mut out = month_summary_by(books, month, zone, basis)?;
    let first = calendar::parse_month(month).ok_or_else(|| {
        crate::error::Error::InvalidInput(format!("month {month}: expected YYYY-MM"))
    })?;
    let (from, to) = Calendar::new(zone).month_bounds(first);
    out.not_executed = not_executed(&automation_runs(books, None), from, to);
    Ok(out)
}
