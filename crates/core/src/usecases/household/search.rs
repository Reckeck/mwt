//! Scenarios of the search area: the pins' values read goals, budgets and the
//! month's spending (other areas', called by method: OD6); `set_pinned` ends
//! in one `vault.transact`.

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::Result;
use crate::household::relations::entity_card_head;
use crate::model::entity_ref::{EntityKind, Pinned};
use crate::money::search::{PinnedView, plan_pins};
use crate::money::worth::FlowClass;
use crate::planning::budget::BudgetStatus;
use crate::planning::flows::flows_between;
use crate::planning::goals::GoalProgress;
use crate::service::UnlockedVault;

use super::relations::store_worth;

/// Spending (or income, for an income category) of the current month in
/// the budget currency.
fn category_month_spent(
    vault: &UnlockedVault,
    key: &str,
    now: i64,
    zone: &str,
) -> (Amount, String) {
    let cal = Calendar::new(zone);
    let (from, to) = cal.month_bounds(cal.date_of(now));
    let currency = crate::planning::budget::budget_currency(&vault.books());
    let spent = flows_between(&vault.books(), &cal, &currency, from, to)
        .into_iter()
        .filter(|flow| {
            matches!(
                flow.class,
                FlowClass::Fixed | FlowClass::Daily | FlowClass::Income
            )
        })
        .filter(|flow| flow.categories.iter().any(|category| category == key))
        .fold(Amount::ZERO, |sum, flow| sum + flow.value);
    (spent.round(2), currency)
}

/// One pin with its head from the card and what its kind adds.
fn pinned_view(
    vault: &UnlockedVault,
    pin: &Pinned,
    goals: &[GoalProgress],
    budgets: &[BudgetStatus],
    (now, zone): (i64, &str),
) -> PinnedView {
    let mut view = PinnedView {
        pin: pin.clone(),
        title: String::new(),
        value: None,
        currency: None,
        pct: None,
        missing: false,
    };
    let target = match pin {
        Pinned::Entity { target } => target,
        Pinned::Filter { name, .. } => {
            view.title = name.clone();
            return view;
        }
    };
    let card = entity_card_head(&vault.doc, target, &store_worth(vault));
    view.title = card.title;
    view.missing = card.missing;
    view.value = card.value;
    view.currency = card.currency;
    match target.kind {
        EntityKind::Goal => {
            if let Some(progress) = goals.iter().find(|progress| progress.goal.id == target.id) {
                view.value = Some(progress.saved);
                view.pct = Some(progress.pct.round(0));
            }
        }
        EntityKind::Budget => {
            if let Some(status) = budgets.iter().find(|status| status.budget.id == target.id) {
                view.value = Some(status.spent);
                view.pct =
                    Some((status.used * Amount::new(rust_decimal::Decimal::ONE_HUNDRED)).round(0));
            }
        }
        EntityKind::Category if !view.missing => {
            let (spent, currency) = category_month_spent(vault, &target.id, now, zone);
            view.value = Some(spent);
            view.currency = Some(currency);
        }
        _ => {}
    }
    view
}

/// The pins with their short values.
pub fn pinned(vault: &UnlockedVault, now: i64, zone: &str) -> Vec<PinnedView> {
    let goals = crate::usecases::planning::goals::goal_progress(&vault.books(), now, zone);
    let budgets = crate::usecases::planning::budget::budget_status(&vault.books(), now, zone);
    vault
        .doc
        .settings
        .pinned
        .iter()
        .map(|pin| pinned_view(vault, pin, &goals, &budgets, (now, zone)))
        .collect()
}

/// Replaces the pins: blanks and repeats are dropped, a filter without an
/// id gets one, the list is capped.
pub fn set_pinned(vault: &mut UnlockedVault, pins: Vec<Pinned>) -> Result<()> {
    let Some(kept) = plan_pins(&vault.doc, pins)? else {
        return Ok(());
    };
    vault.transact(|tx| {
        tx.doc_mut().settings.pinned = kept;
        Ok(())
    })
}
