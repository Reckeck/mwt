//! Scenarios and queries of the budget area. Each scenario decides in
//! `planning::budget`, then ends in one `vault.transact`.
//!
//! The queries that need the automation plan sit here: it comes from
//! `automation_rules`, which sits above `planning`. The queries take the
//! `Books` of one request and the automations as arguments, so a request
//! builds its rates once; only the entry points read the vault.

use std::collections::HashSet;

use chrono::{Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, AUTOMATION_HOUR, Calendar, first_of_month};
use crate::error::Result;
use crate::kernel::books::Books;
use crate::model::planning::draft_to_transaction;
use crate::model::planning::{Automation, Budget, BudgetTarget, UtilityKind};
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::model::vault::DailyBudgetMode;
use crate::money::worth::FlowClass;
use crate::planning::automations::automation_draft;
use crate::planning::budget::{
    BudgetDraft, BudgetStatus, DailyBudget, MonthAllowance, MonthSummary, allowance_of,
    amount_of_days, budget_balance, budget_currency, manual_day_limit, not_executed, period_dates,
    plan_create_budget, plan_delete_budget, plan_update_budget, store_currency, used_of,
};
use crate::planning::flows::{Flow, classify, flow_value, flows_between};
use crate::planning::goals::resolve_scope;
use crate::service::UnlockedVault;
use crate::usecases::planning::automation_rules::{automation_occurrences, automation_runs};
use crate::usecases::planning::automations::automations;

// -- CRUD -------------------------------------------------------------------

pub fn create_budget(vault: &mut UnlockedVault, draft: BudgetDraft) -> Result<Budget> {
    let budget = plan_create_budget(&vault.doc, draft, uuid::Uuid::new_v4().to_string())?;
    vault.transact(|tx| {
        tx.collection::<Budget>().insert(budget.clone())?;
        Ok(budget)
    })
}

pub fn update_budget(vault: &mut UnlockedVault, id: &str, draft: BudgetDraft) -> Result<Budget> {
    let budget = plan_update_budget(&vault.doc, id, draft)?;
    vault.transact(|tx| {
        tx.collection::<Budget>()
            .update(id, |slot| *slot = budget.clone())?;
        Ok(budget)
    })
}

/// Delete a limit; its notices go with it, each with a tombstone, or another
/// device would send them back.
pub fn delete_budget(vault: &mut UnlockedVault, id: &str) -> Result<()> {
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
pub fn budget_status(books: &Books<'_>, now: i64, zone: &str) -> Vec<BudgetStatus> {
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
            let counted: Vec<Flow> = flows_between(
                books,
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
    let mut out = crate::planning::budget::month_summary(books, month, zone)?;
    let first = calendar::parse_month(month).ok_or_else(|| {
        crate::error::Error::InvalidInput(format!("month {month}: expected YYYY-MM"))
    })?;
    let (from, to) = Calendar::new(zone).month_bounds(first);
    out.not_executed = not_executed(&automation_runs(books.doc(), None), from, to);
    Ok(out)
}

// -- the plan of a month from automations -----------------------------------

/// A probe of what an automation books on its next run.
fn probe_of(automation: &Automation, id: &str, at: &str) -> Transaction {
    draft_to_transaction(&automation_draft(automation), id, "", at)
}

/// The store a transfer probe pays into: a loan an automation already pays.
fn paid_loan(probe: &Transaction) -> Option<String> {
    match probe {
        Transaction::Transfer(inner) => inner.to.store_id().map(str::to_string),
        _ => None,
    }
}

/// Planned income and mandatory spending of the month containing `date`:
/// active automations by their class, rent (`Housing`) and loan payments
/// that no automation already covers.
pub(crate) fn month_plan(
    books: &Books<'_>,
    automations: &[Automation],
    cal: &Calendar,
    date: NaiveDate,
    currency: &str,
) -> (Amount, Amount) {
    let book = books.rate_book();
    let first = first_of_month(date);
    let last = calendar::next_month(first) - Duration::days(1);
    let mut income = Amount::ZERO;
    let mut fixed = Amount::ZERO;
    let mut paid_loans: HashSet<String> = HashSet::new();
    for automation in automations.iter().filter(|item| item.active) {
        // Moved off days off: a payday on the 1st may land in the month before.
        let dates = automation_occurrences(
            automation,
            cal,
            cal.start_of(first) - 1,
            cal.start_of(last + Duration::days(1)) - 1,
        )
        .len();
        let probe = probe_of(
            automation,
            &automation.id,
            &calendar::iso(cal.at_local(first, AUTOMATION_HOUR)),
        );
        paid_loans.extend(paid_loan(&probe));
        if dates == 0 {
            continue;
        }
        let class = classify(books, &probe);
        let (value, _) = flow_value(book, &probe, class, currency);
        let total = value * amount_of_days(dates as u32);
        match class {
            FlowClass::Income => income += total,
            FlowClass::Fixed => fixed += total,
            _ => {}
        }
    }
    for item in books.doc().utility_items.iter().filter(|item| item.active) {
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
    for store in &books.doc().stores {
        if store.kind != StoreKind::Liability || store.archived || paid_loans.contains(&store.id) {
            continue;
        }
        let Some(payment) = store
            .liability
            .as_ref()
            .and_then(|terms| terms.monthly_payment)
        else {
            continue;
        };
        let code = store_currency(books, &store.id);
        fixed += book
            .convert(payment, &code, currency, cal.start_of(first))
            .unwrap_or(payment);
    }
    (income, fixed)
}

/// The month-mode allowance of the month containing `date` (plan §7.3).
/// `flows` are that month's.
pub(crate) fn month_allowance(
    books: &Books<'_>,
    automations: &[Automation],
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
    allowance_of(
        (fact(FlowClass::Income), fact(FlowClass::Fixed)),
        month_plan(books, automations, cal, date, currency),
        books.doc().settings.savings_rate,
        date,
    )
}

/// The first planned income date after `today` from active automations.
fn next_income_date(
    books: &Books<'_>,
    automations: &[Automation],
    cal: &Calendar,
    today: NaiveDate,
) -> Option<NaiveDate> {
    let horizon = today + Duration::days(366);
    automations
        .iter()
        .filter(|item| item.active)
        .filter(|automation| {
            let probe = draft_to_transaction(&automation.template, "", "", "");
            classify(books, &probe) == FlowClass::Income
        })
        .filter_map(|automation| {
            automation_occurrences(
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
    books: &Books<'_>,
    automations: &[Automation],
    cal: &Calendar,
    today: NaiveDate,
    end: NaiveDate,
    currency: &str,
) -> Amount {
    let book = books.rate_book();
    let last = end - Duration::days(1);
    let mut total = Amount::ZERO;
    let mut paid_loans: HashSet<String> = HashSet::new();
    for automation in automations.iter().filter(|item| item.active) {
        let probe = probe_of(automation, "", "");
        paid_loans.extend(paid_loan(&probe));
        if classify(books, &probe) != FlowClass::Fixed {
            continue;
        }
        let count = automation_occurrences(
            automation,
            cal,
            cal.start_of(today) - 1,
            cal.start_of(end) - 1,
        )
        .len();
        let (value, _) = flow_value(book, &probe, FlowClass::Fixed, currency);
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
    for item in books.doc().utility_items.iter().filter(|item| item.active) {
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
    for store in &books.doc().stores {
        if store.kind != StoreKind::Liability || store.archived || paid_loans.contains(&store.id) {
            continue;
        }
        let Some(terms) = store.liability.as_ref() else {
            continue;
        };
        if let (Some(payment), Some(due_day)) = (terms.monthly_payment, terms.due_day) {
            let code = store_currency(books, &store.id);
            let value = book
                .convert(payment, &code, currency, cal.start_of(today))
                .unwrap_or(payment);
            total += value * amount_of_days(due_in_window(due_day));
        }
    }
    total
}

/// [`month_plan`] for a caller that holds the vault, not the books.
pub(crate) fn month_plan_of(
    vault: &UnlockedVault,
    cal: &Calendar,
    date: NaiveDate,
    currency: &str,
) -> (Amount, Amount) {
    month_plan(&vault.books(), &automations(vault), cal, date, currency)
}

/// [`month_allowance`] for a caller that holds the vault, not the books.
pub(crate) fn month_allowance_of(
    vault: &UnlockedVault,
    cal: &Calendar,
    date: NaiveDate,
    currency: &str,
    flows: &[Flow],
) -> MonthAllowance {
    month_allowance(
        &vault.books(),
        &automations(vault),
        cal,
        date,
        currency,
        flows,
    )
}

// -- the daily budget -------------------------------------------------------

/// How much can be spent today (plan §7.3), in the budget currency.
pub fn daily_budget(vault: &UnlockedVault, now: i64, zone: &str) -> DailyBudget {
    let books = vault.books();
    let automations = automations(vault);
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let currency = budget_currency(&books);
    let first = first_of_month(today);
    let (from, to) = cal.month_bounds(today);
    let flows = flows_between(&books, &cal, &currency, from, to);
    let spent_on = |date: NaiveDate| {
        flows
            .iter()
            .filter(|flow| flow.class == FlowClass::Daily && flow.date == date)
            .fold(Amount::ZERO, |sum, flow| sum + flow.value)
    };
    let spent_today = spent_on(today);
    let manual_limit = manual_day_limit(&books, &currency, now);
    let mode = books.doc().settings.daily_budget_mode;
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
            let allowance = month_allowance(&books, &automations, &cal, today, &currency, &flows);
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
            let next_income = next_income_date(&books, &automations, &cal, today);
            let end = next_income.unwrap_or_else(|| calendar::next_month(first));
            out.next_income = next_income.map(|date| date.format("%Y-%m-%d").to_string());
            out.days = (end - today).num_days().max(1) as u32;
            let due = fixed_due_between(&books, &automations, &cal, today, end, &currency);
            out.free = budget_balance(&books, &currency, now) - due;
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
