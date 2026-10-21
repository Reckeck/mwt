//! What refers to what (plan J1): the card of any entity and the groups of
//! things linked to it — the goals a wallet is in, the operations an
//! automation made, the rules that fill a category.
//!
//! Read-only: nothing here changes the vault. Workflows keep their graph as
//! JSON, so a store or a category is «used» by a workflow when its id
//! appears quoted in the graph or in the automation it was built from.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::UnlockedVault;
use super::automations::main_amount;
use crate::Amount;
use crate::model::planning::BudgetTarget;
use crate::model::store::StoreScope;
use crate::model::transaction::{Transaction, TxOrigin};
use crate::workflow::model::Workflow;

/// Most items one group lists; `RelationGroup::total` counts all of them.
pub const RELATION_ITEMS_CAP: usize = 20;

/// A kind of thing the app can open a card of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Store,
    Tx,
    Category,
    Contact,
    Goal,
    Budget,
    Automation,
    Workflow,
    Reminder,
    Alert,
    /// `id` is `AssetRef::key()`.
    Asset,
    /// A saved calculator scenario.
    Scenario,
}

/// The address of one entity: what the `?open=kind:id` link carries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
pub struct EntityRef {
    pub kind: EntityKind,
    pub id: String,
}

impl EntityRef {
    pub fn new(kind: EntityKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }
}

/// What a group of related things is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// The wallet a sub-account sits in.
    Parent,
    /// Sub-accounts of a wallet.
    Children,
    /// Wallets: of a goal, holding an asset, touched by an operation.
    Stores,
    Assets,
    Goals,
    Budgets,
    Automations,
    Workflows,
    Reminders,
    Alerts,
    Rules,
    Categories,
    Contacts,
    /// Operations, newest first.
    Transactions,
    /// What made an operation: an automation or a workflow.
    Origin,
    /// The other operations of one movement of money (`flow_id`).
    Flow,
    /// Operations an automation or a workflow made.
    Created,
    /// Calculator scenarios: the one a goal follows.
    Scenarios,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct RelationItem {
    pub target: EntityRef,
    /// The name as stored; empty when the frontend names it itself (a
    /// built-in category, an operation, a budget).
    pub label: String,
    /// A date (RFC3339) for operations and reminders.
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub value: Option<Amount>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct RelationGroup {
    pub kind: RelationKind,
    /// At most [`RELATION_ITEMS_CAP`].
    pub items: Vec<RelationItem>,
    /// How many there are in all.
    pub total: u32,
}

/// The head of an entity's card and what is linked to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct EntityCard {
    pub target: EntityRef,
    /// Empty when the frontend names it (a built-in category, a budget).
    pub title: String,
    /// The main number: a wallet's value, a goal's target, a limit.
    #[serde(default)]
    pub value: Option<Amount>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub archived: bool,
    /// Nothing with this id: deleted, or a link from another vault.
    pub missing: bool,
    pub groups: Vec<RelationGroup>,
    /// The operation itself, for a `tx` card.
    #[serde(default)]
    #[ts(optional)]
    pub transaction: Option<Transaction>,
}

impl RelationItem {
    fn named(kind: EntityKind, id: &str, label: &str) -> Self {
        Self {
            target: EntityRef::new(kind, id),
            label: label.to_string(),
            when: None,
            value: None,
            currency: None,
            archived: false,
        }
    }
}

fn group(kind: RelationKind, mut items: Vec<RelationItem>) -> Option<RelationGroup> {
    if items.is_empty() {
        return None;
    }
    let total = items.len() as u32;
    items.truncate(RELATION_ITEMS_CAP);
    Some(RelationGroup { kind, items, total })
}

fn quoted_in(workflow: &Workflow, needle: &str) -> bool {
    let quoted = format!("\"{needle}\"");
    workflow.graph.contains(&quoted)
        || workflow
            .automation
            .as_ref()
            .and_then(|spec| serde_json::to_string(spec).ok())
            .is_some_and(|json| json.contains(&quoted))
}

fn tx_item(tx: &Transaction) -> RelationItem {
    let mut item = RelationItem::named(EntityKind::Tx, &tx.meta().id, &tx.meta().comment);
    item.when = Some(tx.meta().timestamp.clone());
    item.value = main_amount(tx);
    item
}

fn newest_first(mut txs: Vec<&Transaction>) -> Vec<RelationItem> {
    txs.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
    txs.into_iter().map(tx_item).collect()
}

impl UnlockedVault {
    /// The card of `target`: its head and every non-empty group of links.
    pub fn entity_card(&self, target: &EntityRef) -> EntityCard {
        let mut card = EntityCard {
            target: target.clone(),
            title: String::new(),
            value: None,
            currency: None,
            archived: false,
            missing: false,
            groups: Vec::new(),
            transaction: None,
        };
        let id = target.id.as_str();
        let found = match target.kind {
            EntityKind::Store => self.find_store(id).map(|store| {
                card.title = store.name.clone();
                card.archived = store.archived;
                if let Ok(overview) = self.store_overview(id, true) {
                    card.value = Some(overview.subtree_base);
                    card.currency = Some(overview.base_currency);
                }
            }),
            EntityKind::Tx => self.tx_by_id(id).map(|tx| {
                card.title = tx.meta().comment.clone();
                card.value = main_amount(tx);
                card.transaction = Some(tx.clone());
            }),
            EntityKind::Category => self
                .doc
                .categories
                .iter()
                .find(|category| category.key == id)
                .map(|category| {
                    card.title = category.name.clone().unwrap_or_default();
                    card.archived = category.archived;
                }),
            EntityKind::Contact => self
                .doc
                .contacts
                .iter()
                .find(|contact| contact.id == id)
                .map(|contact| {
                    card.title = contact.name.clone();
                    card.archived = contact.archived;
                }),
            EntityKind::Goal => self
                .doc
                .goals
                .iter()
                .find(|goal| goal.id == id)
                .map(|goal| {
                    card.title = goal.name.clone();
                    card.archived = !goal.active;
                    card.value = Some(goal.target_amount);
                    card.currency = Some(goal.currency.clone());
                }),
            EntityKind::Budget => {
                self.doc
                    .budgets
                    .iter()
                    .find(|budget| budget.id == id)
                    .map(|budget| {
                        card.archived = !budget.active;
                        card.value = Some(budget.limit);
                        card.currency = Some(budget.currency.clone());
                    })
            }
            EntityKind::Automation | EntityKind::Workflow => self
                .doc
                .workflows
                .iter()
                .find(|workflow| workflow.id == id)
                .map(|workflow| {
                    card.title = workflow.name.clone();
                    card.archived = !workflow.active;
                }),
            EntityKind::Reminder => self
                .doc
                .reminders
                .iter()
                .find(|reminder| reminder.id == id)
                .map(|reminder| {
                    card.title = reminder.text.clone();
                    card.archived = reminder.done || reminder.muted;
                }),
            EntityKind::Alert => self
                .doc
                .price_alerts
                .iter()
                .find(|alert| alert.id == id)
                .map(|alert| {
                    card.title = alert.asset.symbol.clone();
                    card.archived = !alert.active;
                    card.value = Some(alert.price);
                    card.currency = Some(alert.currency.clone());
                }),
            EntityKind::Asset => self
                .doc
                .holdings
                .iter()
                .map(|holding| &holding.asset)
                .chain(self.doc.price_alerts.iter().map(|alert| &alert.asset))
                .find(|asset| asset.key() == id)
                .map(|asset| card.title = asset.symbol.clone()),
            EntityKind::Scenario => self.find_calc_scenario(id).map(|scenario| {
                card.title = scenario.name.clone();
                card.value = crate::calc::invest::simulate(&scenario.input)
                    .ok()
                    .map(|result| result.final_nominal);
                card.currency = Some(scenario.input.currency.clone());
            }),
        };
        card.missing = found.is_none();
        if !card.missing {
            card.groups = self.relations(target);
        }
        card
    }

    /// Everything linked to `target`, grouped; empty groups are left out.
    pub fn relations(&self, target: &EntityRef) -> Vec<RelationGroup> {
        let id = target.id.as_str();
        let groups = match target.kind {
            EntityKind::Store => self.store_relations(id),
            EntityKind::Tx => self.tx_relations(id),
            EntityKind::Category => self.category_relations(id),
            EntityKind::Contact => self.contact_relations(id),
            EntityKind::Goal => self.goal_relations(id),
            EntityKind::Budget => self.budget_relations(id),
            EntityKind::Automation | EntityKind::Workflow => self.workflow_relations(id),
            EntityKind::Reminder => self.reminder_relations(id),
            EntityKind::Alert => self.alert_relations(id),
            EntityKind::Asset => self.asset_relations(id),
            EntityKind::Scenario => self.scenario_relations(id),
        };
        groups.into_iter().flatten().collect()
    }

    fn tx_by_id(&self, id: &str) -> Option<&Transaction> {
        self.doc.transactions.iter().find(|tx| tx.meta().id == id)
    }

    fn store_item(&self, id: &str) -> Option<RelationItem> {
        self.find_store(id).map(|store| {
            let mut item = RelationItem::named(EntityKind::Store, &store.id, &store.name);
            item.archived = store.archived;
            item
        })
    }

    /// Workflows split by whether they are automations (built by the form).
    fn workflows_where(&self, used: impl Fn(&Workflow) -> bool) -> [Option<RelationGroup>; 2] {
        let (automations, workflows): (Vec<_>, Vec<_>) = self
            .doc
            .workflows
            .iter()
            .filter(|workflow| used(workflow))
            .partition(|workflow| workflow.automation.is_some());
        let items = |list: Vec<&Workflow>, kind: EntityKind| -> Vec<RelationItem> {
            list.into_iter()
                .map(|workflow| {
                    let mut item = RelationItem::named(kind, &workflow.id, &workflow.name);
                    item.archived = !workflow.active;
                    item
                })
                .collect()
        };
        [
            group(
                RelationKind::Automations,
                items(automations, EntityKind::Automation),
            ),
            group(
                RelationKind::Workflows,
                items(workflows, EntityKind::Workflow),
            ),
        ]
    }

    /// An explicit scope (not «every wallet») that covers `store_id`.
    fn scope_covers(&self, scope: &StoreScope, store_id: &str) -> bool {
        !scope.store_ids.is_empty() && self.resolve_scope(scope).contains(store_id)
    }

    fn reminders_of(&self, id: &str) -> Option<RelationGroup> {
        let items = self
            .doc
            .reminders
            .iter()
            .filter(|reminder| {
                reminder
                    .source
                    .as_ref()
                    .is_some_and(|source| source.id == id)
            })
            .filter(|reminder| !reminder.muted)
            .map(|reminder| {
                let mut item =
                    RelationItem::named(EntityKind::Reminder, &reminder.id, &reminder.text);
                item.when = Some(reminder.due_at.clone());
                item.archived = reminder.done;
                item
            })
            .collect();
        group(RelationKind::Reminders, items)
    }

    fn store_relations(&self, id: &str) -> Vec<Option<RelationGroup>> {
        let Some(store) = self.find_store(id) else {
            return Vec::new();
        };
        let parent = store
            .parent_id
            .as_deref()
            .and_then(|parent| self.store_item(parent))
            .into_iter()
            .collect();
        let children = self
            .doc
            .stores
            .iter()
            .filter(|child| child.parent_id.as_deref() == Some(id))
            .filter_map(|child| self.store_item(&child.id))
            .collect();
        let assets = self
            .doc
            .holdings
            .iter()
            .filter(|holding| holding.store_id == id && !holding.quantity.is_zero())
            .map(|holding| {
                let mut item = RelationItem::named(
                    EntityKind::Asset,
                    &holding.asset.key(),
                    &holding.asset.symbol,
                );
                item.value = Some(holding.quantity);
                item.currency = Some(holding.asset.symbol.clone());
                item
            })
            .collect();
        let goals = self
            .doc
            .goals
            .iter()
            .filter(|goal| self.scope_covers(&goal.scope, id))
            .map(|goal| {
                let mut item = RelationItem::named(EntityKind::Goal, &goal.id, &goal.name);
                item.archived = !goal.active;
                item
            })
            .collect();
        let budgets = self
            .doc
            .budgets
            .iter()
            .filter(|budget| {
                matches!(&budget.target, BudgetTarget::Store(scope) if self.scope_covers(scope, id))
            })
            .map(budget_item)
            .collect();
        let alerts = self
            .doc
            .price_alerts
            .iter()
            .filter(|alert| alert.store_id.as_deref() == Some(id))
            .map(alert_item)
            .collect();
        let txs = newest_first(
            self.doc
                .transactions
                .iter()
                .filter(|tx| tx.touches(id))
                .collect(),
        );
        let [automations, workflows] = self.workflows_where(|workflow| quoted_in(workflow, id));
        vec![
            group(RelationKind::Parent, parent),
            group(RelationKind::Children, children),
            group(RelationKind::Assets, assets),
            group(RelationKind::Goals, goals),
            group(RelationKind::Budgets, budgets),
            automations,
            workflows,
            self.reminders_of(id),
            group(RelationKind::Alerts, alerts),
            group(RelationKind::Transactions, txs),
        ]
    }

    fn tx_relations(&self, id: &str) -> Vec<Option<RelationGroup>> {
        let Some(tx) = self.tx_by_id(id) else {
            return Vec::new();
        };
        let origin_id = match &tx.meta().origin {
            TxOrigin::Automation { automation_id, .. } => Some(automation_id.as_str()),
            TxOrigin::Workflow { workflow_id, .. } => Some(workflow_id.as_str()),
            TxOrigin::Manual | TxOrigin::Import { .. } => None,
        };
        let origin = origin_id
            .and_then(|origin| {
                self.doc
                    .workflows
                    .iter()
                    .find(|workflow| workflow.id == origin)
            })
            .map(|workflow| {
                let kind = if workflow.automation.is_some() {
                    EntityKind::Automation
                } else {
                    EntityKind::Workflow
                };
                RelationItem::named(kind, &workflow.id, &workflow.name)
            })
            .into_iter()
            .collect();
        let flow = match &tx.meta().flow_id {
            Some(flow) => newest_first(
                self.doc
                    .transactions
                    .iter()
                    .filter(|other| {
                        other.meta().id != id && other.meta().flow_id.as_ref() == Some(flow)
                    })
                    .collect(),
            ),
            None => Vec::new(),
        };
        let stores = tx
            .store_ids()
            .iter()
            .filter_map(|store| self.store_item(store))
            .collect();
        let categories = tx
            .meta()
            .categories
            .iter()
            .filter_map(|key| self.category_item(key))
            .collect();
        let mut seen = HashSet::new();
        let contacts = tx
            .contact_ids()
            .into_iter()
            .filter(|contact| seen.insert(*contact))
            .filter_map(|contact| self.contact_item(contact))
            .collect();
        vec![
            group(RelationKind::Origin, origin),
            group(RelationKind::Stores, stores),
            group(RelationKind::Categories, categories),
            group(RelationKind::Contacts, contacts),
            group(RelationKind::Flow, flow),
        ]
    }

    fn category_item(&self, key: &str) -> Option<RelationItem> {
        self.doc
            .categories
            .iter()
            .find(|category| category.key == key)
            .map(|category| {
                let mut item = RelationItem::named(
                    EntityKind::Category,
                    &category.key,
                    category.name.as_deref().unwrap_or_default(),
                );
                item.archived = category.archived;
                item
            })
    }

    fn contact_item(&self, id: &str) -> Option<RelationItem> {
        self.doc
            .contacts
            .iter()
            .find(|contact| contact.id == id)
            .map(|contact| {
                let mut item = RelationItem::named(EntityKind::Contact, &contact.id, &contact.name);
                item.archived = contact.archived;
                item
            })
    }

    fn category_relations(&self, key: &str) -> Vec<Option<RelationGroup>> {
        let budgets = self
            .doc
            .budgets
            .iter()
            .filter(|budget| matches!(&budget.target, BudgetTarget::Category(used) if used == key))
            .map(budget_item)
            .collect();
        let mut rules: Vec<_> = self
            .doc
            .categorization_rules
            .iter()
            .filter(|rule| rule.category == key)
            .collect();
        rules.sort_by_key(|rule| rule.order);
        let rules = rules
            .into_iter()
            .map(|rule| {
                // A rule has no card of its own: it opens the category's.
                let mut item = RelationItem::named(EntityKind::Category, key, &rule.pattern);
                item.archived = !rule.active;
                item
            })
            .collect();
        let txs = newest_first(
            self.doc
                .transactions
                .iter()
                .filter(|tx| tx.meta().categories.iter().any(|used| used == key))
                .collect(),
        );
        let [automations, workflows] = self.workflows_where(|workflow| quoted_in(workflow, key));
        vec![
            group(RelationKind::Budgets, budgets),
            group(RelationKind::Rules, rules),
            automations,
            workflows,
            group(RelationKind::Transactions, txs),
        ]
    }

    fn contact_relations(&self, id: &str) -> Vec<Option<RelationGroup>> {
        let stores = self
            .doc
            .stores
            .iter()
            .filter(|store| store.contact_id.as_deref() == Some(id))
            .filter_map(|store| self.store_item(&store.id))
            .collect();
        let txs = newest_first(
            self.doc
                .transactions
                .iter()
                .filter(|tx| tx.contact_ids().contains(&id))
                .collect(),
        );
        let [automations, workflows] = self.workflows_where(|workflow| quoted_in(workflow, id));
        vec![
            group(RelationKind::Stores, stores),
            automations,
            workflows,
            self.reminders_of(id),
            group(RelationKind::Transactions, txs),
        ]
    }

    fn goal_relations(&self, id: &str) -> Vec<Option<RelationGroup>> {
        let Some(goal) = self.doc.goals.iter().find(|goal| goal.id == id) else {
            return Vec::new();
        };
        let stores = goal
            .scope
            .store_ids
            .iter()
            .filter_map(|store| self.store_item(store))
            .collect();
        let scenario = goal
            .scenario_id
            .as_deref()
            .and_then(|scenario| self.find_calc_scenario(scenario))
            .map(|scenario| {
                RelationItem::named(EntityKind::Scenario, &scenario.id, &scenario.name)
            });
        vec![
            group(RelationKind::Stores, stores),
            group(RelationKind::Scenarios, scenario.into_iter().collect()),
            self.reminders_of(id),
        ]
    }

    fn scenario_relations(&self, id: &str) -> Vec<Option<RelationGroup>> {
        let Some(scenario) = self.find_calc_scenario(id) else {
            return Vec::new();
        };
        let goals = self
            .doc
            .goals
            .iter()
            .filter(|goal| goal.scenario_id.as_deref() == Some(id))
            .map(|goal| {
                let mut item = RelationItem::named(EntityKind::Goal, &goal.id, &goal.name);
                item.archived = !goal.active;
                item
            })
            .collect();
        let stores = scenario
            .source
            .iter()
            .flat_map(|source| &source.scope.store_ids)
            .filter_map(|store| self.store_item(store))
            .collect();
        vec![
            group(RelationKind::Goals, goals),
            group(RelationKind::Stores, stores),
        ]
    }

    fn budget_relations(&self, id: &str) -> Vec<Option<RelationGroup>> {
        let Some(budget) = self.doc.budgets.iter().find(|budget| budget.id == id) else {
            return Vec::new();
        };
        match &budget.target {
            BudgetTarget::Total => Vec::new(),
            BudgetTarget::Category(key) => vec![group(
                RelationKind::Categories,
                self.category_item(key).into_iter().collect(),
            )],
            BudgetTarget::Store(scope) => vec![group(
                RelationKind::Stores,
                scope
                    .store_ids
                    .iter()
                    .filter_map(|store| self.store_item(store))
                    .collect(),
            )],
        }
    }

    fn workflow_relations(&self, id: &str) -> Vec<Option<RelationGroup>> {
        let Some(workflow) = self.doc.workflows.iter().find(|workflow| workflow.id == id) else {
            return Vec::new();
        };
        let created = newest_first(
            self.doc
                .transactions
                .iter()
                .filter(|tx| match &tx.meta().origin {
                    TxOrigin::Automation { automation_id, .. } => automation_id == id,
                    TxOrigin::Workflow { workflow_id, .. } => workflow_id == id,
                    TxOrigin::Manual | TxOrigin::Import { .. } => false,
                })
                .collect(),
        );
        let stores = self
            .doc
            .stores
            .iter()
            .filter(|store| quoted_in(workflow, &store.id))
            .filter_map(|store| self.store_item(&store.id))
            .collect();
        let categories = self
            .doc
            .categories
            .iter()
            .filter(|category| quoted_in(workflow, &category.key))
            .filter_map(|category| self.category_item(&category.key))
            .collect();
        vec![
            group(RelationKind::Stores, stores),
            group(RelationKind::Categories, categories),
            group(RelationKind::Created, created),
        ]
    }

    fn reminder_relations(&self, id: &str) -> Vec<Option<RelationGroup>> {
        let Some(source) = self
            .doc
            .reminders
            .iter()
            .find(|reminder| reminder.id == id)
            .and_then(|reminder| reminder.source.as_ref())
        else {
            return Vec::new();
        };
        let source_id = source.id.as_str();
        let stores = self.store_item(source_id).into_iter().collect();
        let goals = self
            .doc
            .goals
            .iter()
            .filter(|goal| goal.id == source_id)
            .map(|goal| RelationItem::named(EntityKind::Goal, &goal.id, &goal.name))
            .collect();
        let contacts = self.contact_item(source_id).into_iter().collect();
        vec![
            group(RelationKind::Stores, stores),
            group(RelationKind::Goals, goals),
            group(RelationKind::Contacts, contacts),
        ]
    }

    fn alert_relations(&self, id: &str) -> Vec<Option<RelationGroup>> {
        let Some(alert) = self.doc.price_alerts.iter().find(|alert| alert.id == id) else {
            return Vec::new();
        };
        let asset = vec![RelationItem::named(
            EntityKind::Asset,
            &alert.asset.key(),
            &alert.asset.symbol,
        )];
        let stores = alert
            .store_id
            .as_deref()
            .and_then(|store| self.store_item(store))
            .into_iter()
            .collect();
        vec![
            group(RelationKind::Assets, asset),
            group(RelationKind::Stores, stores),
        ]
    }

    fn asset_relations(&self, key: &str) -> Vec<Option<RelationGroup>> {
        let stores = self
            .doc
            .holdings
            .iter()
            .filter(|holding| holding.asset.key() == key && !holding.quantity.is_zero())
            .filter_map(|holding| {
                self.store_item(&holding.store_id).map(|mut item| {
                    item.value = Some(holding.quantity);
                    item.currency = Some(holding.asset.symbol.clone());
                    item
                })
            })
            .collect();
        let alerts = self
            .doc
            .price_alerts
            .iter()
            .filter(|alert| alert.asset.key() == key)
            .map(alert_item)
            .collect();
        vec![
            group(RelationKind::Stores, stores),
            group(RelationKind::Alerts, alerts),
        ]
    }
}

fn budget_item(budget: &crate::model::planning::Budget) -> RelationItem {
    let mut item = RelationItem::named(EntityKind::Budget, &budget.id, "");
    item.value = Some(budget.limit);
    item.currency = Some(budget.currency.clone());
    item.archived = !budget.active;
    item
}

fn alert_item(alert: &crate::model::planning::PriceAlert) -> RelationItem {
    let mut item = RelationItem::named(EntityKind::Alert, &alert.id, &alert.asset.symbol);
    item.value = Some(alert.price);
    item.currency = Some(alert.currency.clone());
    item.archived = !alert.active;
    item
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::planning::{CategorizationRule, Goal};
    use crate::service::test_support::*;

    fn goal_on(store_id: &str) -> Goal {
        Goal {
            id: "g1".into(),
            name: "Отпуск".into(),
            active: true,
            target_amount: amount("1000"),
            currency: "USD".into(),
            starts_at: "2026-01-01".into(),
            deadline: None,
            priority: 3,
            monthly_commitment: None,
            scope: StoreScope {
                store_ids: vec![store_id.into()],
                include_children: true,
            },
            created_at: "2026-01-01T00:00:00Z".into(),
            scenario_id: None,
            expected_return: None,
        }
    }

    fn kinds(groups: &[RelationGroup]) -> Vec<RelationKind> {
        groups.iter().map(|group| group.kind).collect()
    }

    #[test]
    fn a_wallet_lists_its_goals_and_operations() {
        let (_dir, mut vault) = setup();
        let binance = vault.doc.stores[0].id.clone();
        vault
            .doc
            .transactions
            .push(income(&binance, "100", "зарплата"));
        vault.doc.goals.push(goal_on(&binance));
        let card = vault.entity_card(&EntityRef::new(EntityKind::Store, &binance));
        assert!(!card.missing);
        let goals = card
            .groups
            .iter()
            .find(|group| group.kind == RelationKind::Goals)
            .expect("goal group");
        assert_eq!(
            goals.items[0].target,
            EntityRef::new(EntityKind::Goal, "g1")
        );
        assert!(kinds(&card.groups).contains(&RelationKind::Transactions));
        // No budgets, alerts or children: their groups are not sent.
        assert!(!kinds(&card.groups).contains(&RelationKind::Budgets));
        assert!(!kinds(&card.groups).contains(&RelationKind::Children));
    }

    #[test]
    fn a_goal_over_every_wallet_is_not_listed_on_each() {
        let (_dir, mut vault) = setup();
        let binance = vault.doc.stores[0].id.clone();
        let mut goal = goal_on(&binance);
        goal.scope.store_ids.clear();
        vault.doc.goals.push(goal);
        let groups = vault.relations(&EntityRef::new(EntityKind::Store, &binance));
        assert!(!kinds(&groups).contains(&RelationKind::Goals));
    }

    #[test]
    fn a_category_lists_its_rules_and_operations() {
        let (_dir, mut vault) = setup();
        vault.doc.categorization_rules.push(CategorizationRule {
            id: "r1".into(),
            pattern: "пятёрочка".into(),
            category: "groceries".into(),
            active: true,
            order: 0,
        });
        let binance = vault.doc.stores[0].id.clone();
        let mut tx = payment(&binance, "10", "хлеб");
        tx.meta_mut().id = "t-food".into();
        vault.doc.transactions.push(tx);
        let groups = vault.relations(&EntityRef::new(EntityKind::Category, "groceries"));
        assert_eq!(
            kinds(&groups),
            vec![RelationKind::Rules, RelationKind::Transactions]
        );
        assert_eq!(groups[0].items[0].label, "пятёрочка");
        assert_eq!(groups[1].items[0].target.id, "t-food");
    }

    #[test]
    fn an_operation_leads_to_the_workflow_that_made_it_and_its_flow() {
        let (_dir, mut vault) = setup();
        vault.doc.workflows.push(Workflow {
            id: "wf".into(),
            name: "Зарплата".into(),
            description: String::new(),
            active: true,
            graph: "{}".into(),
            created_at: String::new(),
            updated_at: String::new(),
            react_to_workflows: false,
            max_amount: None,
            versions: Vec::new(),
            tests: Vec::new(),
            draft: None,
            automation: None,
        });
        let binance = vault.doc.stores[0].id.clone();
        let mut first = income(&binance, "100", "зарплата");
        first.meta_mut().id = "t1".into();
        first.meta_mut().flow_id = Some("flow".into());
        first.meta_mut().origin = TxOrigin::Workflow {
            workflow_id: "wf".into(),
            run_id: "run".into(),
            planned_at: None,
            estimated: false,
        };
        let mut second = first.clone();
        second.meta_mut().id = "t2".into();
        vault.doc.transactions.extend([first, second]);

        let groups = vault.relations(&EntityRef::new(EntityKind::Tx, "t1"));
        let origin = &groups[0];
        assert_eq!(origin.kind, RelationKind::Origin);
        assert_eq!(
            origin.items[0].target,
            EntityRef::new(EntityKind::Workflow, "wf")
        );
        let flow = groups
            .iter()
            .find(|group| group.kind == RelationKind::Flow)
            .unwrap();
        assert_eq!(flow.total, 1);
        assert_eq!(flow.items[0].target.id, "t2");

        let made = vault.relations(&EntityRef::new(EntityKind::Workflow, "wf"));
        let created = made
            .iter()
            .find(|group| group.kind == RelationKind::Created)
            .unwrap();
        assert_eq!(created.total, 2);
    }

    #[test]
    fn a_link_to_nothing_is_a_missing_card() {
        let (_dir, vault) = setup();
        let card = vault.entity_card(&EntityRef::new(EntityKind::Goal, "gone"));
        assert!(card.missing);
        assert!(card.groups.is_empty());
    }

    #[test]
    fn groups_are_capped_but_count_everything() {
        let (_dir, mut vault) = setup();
        let binance = vault.doc.stores[0].id.clone();
        let sample = income(&binance, "1", "");
        for index in 0..30 {
            let mut tx = sample.clone();
            tx.meta_mut().id = format!("bulk-{index}");
            vault.doc.transactions.push(tx);
        }
        let groups = vault.relations(&EntityRef::new(EntityKind::Store, &binance));
        let txs = groups
            .iter()
            .find(|group| group.kind == RelationKind::Transactions)
            .unwrap();
        assert_eq!(txs.items.len(), RELATION_ITEMS_CAP);
        assert!(txs.total as usize > RELATION_ITEMS_CAP);
    }
}
