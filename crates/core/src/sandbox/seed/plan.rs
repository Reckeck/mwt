//! The demo plan: three limits (one of them over), two goals with a wallet and
//! three automations, so every card of "Money" and "Automations" has something to try.

use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;

use crate::model::planning::{BudgetPeriod, BudgetTarget};
use crate::model::store::StoreScope;
use crate::service::{BudgetDraft, GoalDraft, UnlockedVault};
use crate::{Amount, Result};

use super::ids;
use super::wallets::Wallets;

fn dollars(whole: i64) -> Amount {
    Amount::new(Decimal::new(whole, 0))
}

fn limit(category: &str, whole: i64) -> BudgetDraft {
    BudgetDraft {
        active: true,
        target: BudgetTarget::Category(category.to_string()),
        period: BudgetPeriod::Month,
        limit: dollars(whole),
        currency: "usd".into(),
        warn: true,
    }
}

fn goal(
    name: &str,
    target: i64,
    store: &str,
    today: NaiveDate,
    months: i64,
    per_month: i64,
) -> GoalDraft {
    GoalDraft {
        name: name.to_string(),
        active: true,
        target_amount: dollars(target),
        currency: "usd".into(),
        starts_at: format!("{}T00:00:00Z", today - Duration::days(365)),
        deadline: Some(format!("{}T00:00:00Z", today + Duration::days(30 * months))),
        priority: 3,
        monthly_commitment: Some(dollars(per_month)),
        scope: StoreScope {
            store_ids: vec![store.to_string()],
            include_children: true,
        },
        scenario_id: None,
        expected_return: None,
    }
}

/// Three limits (taxi is over), two goals and three automations, with fixed ids.
pub fn add(vault: &mut UnlockedVault, wallets: &Wallets, today: NaiveDate) -> Result<()> {
    let mut number = 0u128;
    let mut next = || {
        number += 1;
        ids::plan(number)
    };
    let mut budget_ids = Vec::new();
    for draft in [
        limit("groceries", 450),
        limit("food_delivery", 80),
        limit("taxi", 25),
    ] {
        let made = vault.create_budget(draft)?;
        let id = next();
        if let Some(budget) = vault
            .doc
            .budgets
            .iter_mut()
            .find(|budget| budget.id == made.id)
        {
            budget.id = id.clone();
        }
        budget_ids.push(id);
    }
    for draft in [
        goal(
            "Подушка безопасности",
            12_000,
            &wallets.savings,
            today,
            14,
            150,
        ),
        goal("Отпуск на море", 3_000, &wallets.savings, today, 8, 200),
    ] {
        let made = vault.create_goal(draft)?;
        let id = next();
        if let Some(goal) = vault.doc.goals.iter_mut().find(|goal| goal.id == made.id) {
            goal.id = id;
            goal.created_at = format!("{today}T00:00:00Z");
        }
    }
    let now = today
        .and_hms_opt(8, 0, 0)
        .unwrap_or_default()
        .and_utc()
        .timestamp();
    let params = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    };
    let cards = [
        (
            "pay_yourself_first",
            params(&[
                ("category", "salary"),
                ("pct", "10"),
                ("target", &wallets.savings),
            ]),
        ),
        (
            "budget_pace",
            params(&[("budget", &budget_ids[2]), ("margin", "10")]),
        ),
        (
            "subscription_watch",
            params(&[("category", "subscriptions")]),
        ),
    ];
    for (template, fields) in cards {
        let saved = vault.workflow_from_template(template, &fields, "ru", now)?;
        let id = next();
        if let Some(workflow) = vault
            .doc
            .workflows
            .iter_mut()
            .find(|item| item.id == saved.workflow.id)
        {
            workflow.id = id.clone();
        }
        for state in vault
            .doc
            .workflow_state
            .iter_mut()
            .filter(|state| state.workflow_id == saved.workflow.id)
        {
            state.workflow_id = id.clone();
        }
    }
    Ok(())
}
