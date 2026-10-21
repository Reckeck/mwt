//! What the palette finds and what the sidebar keeps pinned (plan J2).

use serde::{Deserialize, Serialize};

use super::UnlockedVault;
use super::automations::main_amount;
use super::worth::FlowClass;
use crate::Amount;
use crate::calendar::Calendar;
use crate::error::{Error, Result};
use crate::model::entity_ref::{EntityKind, EntityRef, Pinned};
use crate::model::planning::BudgetTarget;
use crate::model::transaction::Transaction;

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

/// `"1 234,50"` -> `"1234.50"`: how an amount typed in the search compares.
fn numeric_query(query: &str) -> Option<String> {
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

impl UnlockedVault {
    /// Every entity by name, for the palette; wallets are left to the store tree.
    pub fn search_index(&self) -> Vec<SearchEntry> {
        let entry = |kind: EntityKind, id: &str, label: &str, archived: bool| SearchEntry {
            target: EntityRef::new(kind, id),
            label: label.to_string(),
            subject: None,
            archived,
        };
        let mut out = Vec::new();
        out.extend(
            self.doc
                .goals
                .iter()
                .map(|goal| entry(EntityKind::Goal, &goal.id, &goal.name, !goal.active)),
        );
        out.extend(self.doc.budgets.iter().map(|budget| {
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
        }));
        out.extend(self.doc.categories.iter().map(|category| {
            entry(
                EntityKind::Category,
                &category.key,
                category.name.as_deref().unwrap_or_default(),
                category.archived,
            )
        }));
        out.extend(self.doc.contacts.iter().map(|contact| {
            entry(
                EntityKind::Contact,
                &contact.id,
                &contact.name,
                contact.archived,
            )
        }));
        out.extend(self.doc.workflows.iter().map(|workflow| {
            let kind = if workflow.automation.is_some() {
                EntityKind::Automation
            } else {
                EntityKind::Workflow
            };
            entry(kind, &workflow.id, &workflow.name, !workflow.active)
        }));
        out.extend(
            self.doc
                .reminders
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
        out.extend(self.doc.price_alerts.iter().map(|alert| {
            entry(
                EntityKind::Alert,
                &alert.id,
                &alert.asset.symbol,
                !alert.active,
            )
        }));
        let mut assets: Vec<String> = Vec::new();
        for holding in &self.doc.holdings {
            let key = holding.asset.key();
            if holding.quantity.is_zero() || assets.contains(&key) {
                continue;
            }
            out.push(entry(EntityKind::Asset, &key, &holding.asset.symbol, false));
            assets.push(key);
        }
        out.extend(
            self.doc
                .calc_scenarios
                .iter()
                .map(|scenario| entry(EntityKind::Scenario, &scenario.id, &scenario.name, false)),
        );
        out
    }

    /// Operations whose comment, wallet, contact or amount match `query`, or
    /// that carry one of `categories` (the frontend matches the translated
    /// names of built-in categories itself). Newest first.
    pub fn search_transactions(
        &self,
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
        let contact_ids: Vec<&str> = self
            .doc
            .contacts
            .iter()
            .filter(|contact| named(&contact.name))
            .map(|contact| contact.id.as_str())
            .collect();
        let store_ids: Vec<&str> = self
            .doc
            .stores
            .iter()
            .filter(|store| named(&store.name))
            .map(|store| store.id.as_str())
            .collect();
        let custom_categories: Vec<&str> = self
            .doc
            .categories
            .iter()
            .filter(|category| category.name.as_deref().is_some_and(named))
            .map(|category| category.key.as_str())
            .collect();
        let mut found: Vec<&Transaction> = self
            .doc
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
                        main_amount(tx).is_some_and(|amount| {
                            amount.abs().to_string().starts_with(number.as_str())
                        })
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

    /// The pins with their short values.
    pub fn pinned(&self, now: i64, zone: &str) -> Vec<PinnedView> {
        let goals = self.goal_progress(now, zone);
        let budgets = self.budget_status(now, zone);
        self.doc
            .settings
            .pinned
            .iter()
            .map(|pin| {
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
                let card = self.entity_card_head(target);
                view.title = card.title;
                view.missing = card.missing;
                view.value = card.value;
                view.currency = card.currency;
                match target.kind {
                    EntityKind::Goal => {
                        if let Some(progress) =
                            goals.iter().find(|progress| progress.goal.id == target.id)
                        {
                            view.value = Some(progress.saved);
                            view.pct = Some(progress.pct.round(0));
                        }
                    }
                    EntityKind::Budget => {
                        if let Some(status) =
                            budgets.iter().find(|status| status.budget.id == target.id)
                        {
                            view.value = Some(status.spent);
                            view.pct = Some(
                                (status.used * Amount::new(rust_decimal::Decimal::ONE_HUNDRED))
                                    .round(0),
                            );
                        }
                    }
                    EntityKind::Category if !view.missing => {
                        let (spent, currency) = self.category_month_spent(&target.id, now, zone);
                        view.value = Some(spent);
                        view.currency = Some(currency);
                    }
                    _ => {}
                }
                view
            })
            .collect()
    }

    /// Replaces the pins: blanks and repeats are dropped, a filter without an
    /// id gets one, the list is capped.
    pub fn set_pinned(&mut self, pins: Vec<Pinned>) -> Result<()> {
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
        if kept == self.doc.settings.pinned {
            return Ok(());
        }
        self.doc.settings.pinned = kept;
        self.persist()
    }

    /// Spending (or income, for an income category) of the current month in
    /// the budget currency.
    fn category_month_spent(&self, key: &str, now: i64, zone: &str) -> (Amount, String) {
        let cal = Calendar::new(zone);
        let (from, to) = cal.month_bounds(cal.date_of(now));
        let currency = self.budget_currency();
        let spent = self
            .flows_between(&cal, &currency, from, to)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::now_secs;
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;

    #[test]
    fn numbers_compare_without_spaces_and_commas() {
        assert_eq!(numeric_query("1 234,5").as_deref(), Some("1234.5"));
        assert_eq!(numeric_query("кофе"), None);
        assert_eq!(numeric_query("."), None);
    }

    #[test]
    fn operations_are_found_by_comment_amount_and_wallet() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Tinkoff", StoreKind::Bank, None);
        vault
            .add_transaction(income(&card, "1000", "salary"))
            .unwrap();
        vault
            .add_transaction(payment(&card, "230", "Coffee"))
            .unwrap();
        let by_comment = vault.search_transactions("coff", &[], 10);
        assert_eq!(by_comment.len(), 1);
        let by_amount = vault.search_transactions("23", &[], 10);
        assert_eq!(by_amount.len(), 1);
        assert_eq!(vault.search_transactions("tink", &[], 10).len(), 2);
        assert!(vault.search_transactions("  ", &[], 10).is_empty());
    }

    #[test]
    fn pins_are_deduplicated_and_valued() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        vault
            .add_transaction(income(&card, "500", "opening"))
            .unwrap();
        let wallet = Pinned::Entity {
            target: EntityRef::new(EntityKind::Store, &card),
        };
        let filter = Pinned::Filter {
            id: String::new(),
            name: " Food ".into(),
            query: "?category=groceries".into(),
        };
        vault
            .set_pinned(vec![wallet.clone(), wallet.clone(), filter])
            .unwrap();
        let pins = vault.pinned(now_secs(), "UTC");
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0].title, "Card");
        assert!(pins[0].value.is_some());
        let Pinned::Filter { id, name, query } = &pins[1].pin else {
            panic!("a filter");
        };
        assert!(!id.is_empty());
        assert_eq!(name, "Food");
        assert_eq!(query, "category=groceries");

        let gone = Pinned::Entity {
            target: EntityRef::new(EntityKind::Goal, "nope"),
        };
        vault.set_pinned(vec![gone]).unwrap();
        assert!(vault.pinned(now_secs(), "UTC")[0].missing);
        let unnamed = Pinned::Filter {
            id: String::new(),
            name: "  ".into(),
            query: String::new(),
        };
        assert!(matches!(
            vault.set_pinned(vec![unnamed]),
            Err(Error::InvalidInput(_))
        ));
    }
}
