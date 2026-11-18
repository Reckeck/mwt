//! What the palette finds and what the sidebar keeps pinned (plan J2).
//!
//! Pure part of the area: the index and the operation search read a document;
//! the pins are checked here and written by the scenario in
//! `usecases::money::search`, which also values them (goals, budgets and the
//! month's spending are other areas').

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::entity_ref::{EntityKind, EntityRef, Pinned};
use crate::model::planning::BudgetTarget;
use crate::model::transaction::Transaction;
use crate::model::vault::VaultDoc;

/// Most pins the sidebar keeps.
pub const PINNED_CAP: usize = 30;
/// Most operations one search returns.
pub const SEARCH_TX_CAP: usize = 50;

/// One entity the palette can find.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SearchEntry {
    pub target: EntityRef,
    /// The stored name; empty when the frontend names it (a built-in category).
    pub label: String,
    /// What a nameless entity is about: a budget's category or wallet.
    #[serde(default)]
    pub subject: Option<EntityRef>,
    #[serde(default)]
    pub archived: bool,
}

/// A pin with what the sidebar shows next to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PinnedView {
    pub pin: Pinned,
    /// The stored name; empty when the frontend names it.
    pub title: String,
    /// A wallet's value, a category's spending this month, a limit's spending.
    #[serde(default)]
    pub value: Option<Amount>,
    #[serde(default)]
    pub currency: Option<String>,
    /// A goal's progress or a limit's use, percent.
    #[serde(default)]
    pub pct: Option<Amount>,
    /// The entity is gone (deleted, or pinned on another vault).
    pub missing: bool,
}

/// The amount a top-up, a payment or a transfer moves (an exchange: what it gives).
///
/// The automations area still keeps its own copy (`service::automations`);
/// the pure areas cannot name the service, so this is the one they share.
pub fn main_amount(tx: &Transaction) -> Option<Amount> {
    match tx {
        Transaction::Topup(inner) => Some(inner.amount),
        Transaction::Payment(inner) => Some(inner.amount),
        Transaction::Transfer(inner) => Some(inner.amount),
        Transaction::Convert(inner) => Some(inner.from.amount),
        _ => None,
    }
}

/// `"1 234,50"` -> `"1234.50"`: how an amount typed in the search compares.
pub fn numeric_query(query: &str) -> Option<String> {
    let compact: String = query
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '\u{a0}')
        .map(|ch| if ch == ',' { '.' } else { ch })
        .collect();
    let valid = !compact.is_empty()
        && compact.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
        && compact.chars().any(|ch| ch.is_ascii_digit());
    valid.then_some(compact)
}

fn entry(kind: EntityKind, id: &str, label: &str, archived: bool) -> SearchEntry {
    SearchEntry {
        target: EntityRef::new(kind, id),
        label: label.to_string(),
        subject: None,
        archived,
    }
}

/// A budget is nameless: it points at its category or wallet instead.
fn budget_entry(budget: &crate::model::planning::Budget) -> SearchEntry {
    let mut item = entry(EntityKind::Budget, &budget.id, "", !budget.active);
    item.subject = match &budget.target {
        BudgetTarget::Category(key) => Some(EntityRef::new(EntityKind::Category, key)),
        BudgetTarget::Store(scope) => scope
            .store_ids
            .first()
            .map(|id| EntityRef::new(EntityKind::Store, id)),
        BudgetTarget::Total => None,
    };
    item
}

/// Every entity by name, for the palette; wallets are left to the store tree.
pub fn search_index(doc: &VaultDoc) -> Vec<SearchEntry> {
    let mut out = Vec::new();
    out.extend(
        doc.goals
            .iter()
            .map(|goal| entry(EntityKind::Goal, &goal.id, &goal.name, !goal.active)),
    );
    out.extend(doc.budgets.iter().map(budget_entry));
    out.extend(doc.categories.iter().map(|category| {
        entry(
            EntityKind::Category,
            &category.key,
            category.name.as_deref().unwrap_or_default(),
            category.archived,
        )
    }));
    out.extend(doc.contacts.iter().map(|contact| {
        entry(
            EntityKind::Contact,
            &contact.id,
            &contact.name,
            contact.archived,
        )
    }));
    out.extend(doc.workflows.iter().map(|workflow| {
        let kind = if workflow.automation.is_some() {
            EntityKind::Automation
        } else {
            EntityKind::Workflow
        };
        entry(kind, &workflow.id, &workflow.name, !workflow.active)
    }));
    out.extend(
        doc.reminders
            .iter()
            .filter(|reminder| !reminder.muted)
            .map(|reminder| {
                entry(
                    EntityKind::Reminder,
                    &reminder.id,
                    &reminder.text,
                    reminder.done,
                )
            }),
    );
    out.extend(doc.price_alerts.iter().map(|alert| {
        entry(
            EntityKind::Alert,
            &alert.id,
            &alert.asset.symbol,
            !alert.active,
        )
    }));
    let mut assets: Vec<String> = Vec::new();
    for holding in &doc.holdings {
        let key = holding.asset.key();
        if holding.quantity.is_zero() || assets.contains(&key) {
            continue;
        }
        out.push(entry(EntityKind::Asset, &key, &holding.asset.symbol, false));
        assets.push(key);
    }
    out.extend(
        doc.calc_scenarios
            .iter()
            .map(|scenario| entry(EntityKind::Scenario, &scenario.id, &scenario.name, false)),
    );
    out
}

/// Operations whose comment, wallet, contact or amount match `query`, or
/// that carry one of `categories` (the frontend matches the translated
/// names of built-in categories itself). Newest first.
pub fn search_transactions(
    doc: &VaultDoc,
    query: &str,
    categories: &[String],
    limit: usize,
) -> Vec<Transaction> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() && categories.is_empty() {
        return Vec::new();
    }
    let number = numeric_query(&needle);
    let named = |name: &str| !needle.is_empty() && name.to_lowercase().contains(&needle);
    let contact_ids: Vec<&str> = doc
        .contacts
        .iter()
        .filter(|contact| named(&contact.name))
        .map(|contact| contact.id.as_str())
        .collect();
    let store_ids: Vec<&str> = doc
        .stores
        .iter()
        .filter(|store| named(&store.name))
        .map(|store| store.id.as_str())
        .collect();
    let custom_categories: Vec<&str> = doc
        .categories
        .iter()
        .filter(|category| category.name.as_deref().is_some_and(named))
        .map(|category| category.key.as_str())
        .collect();
    let mut found: Vec<&Transaction> = doc
        .transactions
        .iter()
        .filter(|tx| {
            let meta = tx.meta();
            named(&meta.comment)
                || meta.categories.iter().any(|key| {
                    categories.contains(key) || custom_categories.contains(&key.as_str())
                })
                || meta
                    .contact_id
                    .as_deref()
                    .is_some_and(|id| contact_ids.contains(&id))
                || tx
                    .store_ids()
                    .iter()
                    .any(|id| store_ids.contains(&id.as_str()))
                || number.as_ref().is_some_and(|number| {
                    main_amount(tx)
                        .is_some_and(|amount| amount.abs().to_string().starts_with(number.as_str()))
                })
        })
        .collect();
    found.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
    found
        .into_iter()
        .take(limit.clamp(1, SEARCH_TX_CAP))
        .cloned()
        .collect()
}

/// The pins to keep for `pins`: blanks and repeats are dropped, a filter
/// without an id gets one, the list is capped. `None` when the stored list
/// already is that (nothing to write).
pub fn plan_pins(doc: &VaultDoc, pins: Vec<Pinned>) -> Result<Option<Vec<Pinned>>> {
    let mut kept: Vec<Pinned> = Vec::new();
    for pin in pins {
        let pin = match pin {
            Pinned::Entity { target } if target.id.trim().is_empty() => continue,
            Pinned::Filter { id, name, query } => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Err(Error::InvalidInput("a saved filter needs a name".into()));
                }
                let id = if id.trim().is_empty() {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    id
                };
                Pinned::Filter {
                    id,
                    name,
                    query: query.trim().trim_start_matches('?').to_string(),
                }
            }
            entity => entity,
        };
        if !kept.contains(&pin) {
            kept.push(pin);
        }
    }
    kept.truncate(PINNED_CAP);
    Ok((kept != doc.settings.pinned).then_some(kept))
}
