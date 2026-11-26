//! What refers to what (plan J1): the card of any entity and the groups of
//! things linked to it — the goals a wallet is in, the operations an
//! automation made, the rules that fill a category.
//!
//! Read-only: nothing here changes the vault. Workflows keep their graph as
//! JSON, so a store or a category is «used» by a workflow when its id
//! appears quoted in the graph or in the automation it was built from.
//!
//! Pure part of the area: every function reads a document. What a wallet is
//! worth comes from outside (`StoreWorth`), since the portfolio area owns it.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use mwt_base::Amount;
use mwt_model::StoreScope;
use mwt_model::VaultDoc;
use mwt_model::Workflow;
use mwt_model::{Budget, BudgetTarget, CalcScenario, PriceAlert};
use mwt_model::{Transaction, TxOrigin, made_by};
use mwt_money::search::main_amount;
use mwt_money::tree::{find_store, resolve_scope};

/// Most items one group lists; `RelationGroup::total` counts all of them.
pub const RELATION_ITEMS_CAP: usize = 20;

pub use mwt_model::{EntityKind, EntityRef};

/// A wallet's value with its subtree and the currency, as the card shows it;
/// `None` when the portfolio cannot tell. Asked for by wallet id.
pub type StoreWorth<'worth> = &'worth dyn Fn(&str) -> Option<(Amount, String)>;

/// What a group of related things is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RelationGroup {
    pub kind: RelationKind,
    /// At most [`RELATION_ITEMS_CAP`].
    pub items: Vec<RelationItem>,
    /// How many there are in all.
    pub total: u32,
}

/// The head of an entity's card and what is linked to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    #[cfg_attr(feature = "ts", ts(optional))]
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

fn find_calc_scenario<'doc>(doc: &'doc VaultDoc, id: &str) -> Option<&'doc CalcScenario> {
    doc.calc_scenarios.iter().find(|scenario| scenario.id == id)
}

/// The card of `target`: its head and every non-empty group of links.
pub fn card_of(doc: &VaultDoc, target: &EntityRef, store_worth: StoreWorth) -> EntityCard {
    let mut card = entity_card_head(doc, target, store_worth);
    if !card.missing {
        card.groups = relations(doc, target);
    }
    card
}

/// The card without its groups: name, main number, whether it exists.
pub fn entity_card_head(doc: &VaultDoc, target: &EntityRef, store_worth: StoreWorth) -> EntityCard {
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
        EntityKind::Store => find_store(doc, id).map(|store| {
            card.title = store.name.clone();
            card.archived = store.archived;
            if let Some((value, currency)) = store_worth(id) {
                card.value = Some(value);
                card.currency = Some(currency);
            }
        }),
        EntityKind::Tx => tx_by_id(doc, id).map(|tx| {
            card.title = tx.meta().comment.clone();
            card.value = main_amount(tx);
            card.transaction = Some(tx.clone());
        }),
        EntityKind::Category => doc
            .categories
            .iter()
            .find(|category| category.key == id)
            .map(|category| {
                card.title = category.name.clone().unwrap_or_default();
                card.archived = category.archived;
            }),
        EntityKind::Contact => {
            doc.contacts
                .iter()
                .find(|contact| contact.id == id)
                .map(|contact| {
                    card.title = contact.name.clone();
                    card.archived = contact.archived;
                })
        }
        EntityKind::Goal => doc.goals.iter().find(|goal| goal.id == id).map(|goal| {
            card.title = goal.name.clone();
            card.archived = !goal.active;
            card.value = Some(goal.target_amount);
            card.currency = Some(goal.currency.clone());
        }),
        EntityKind::Budget => doc
            .budgets
            .iter()
            .find(|budget| budget.id == id)
            .map(|budget| {
                card.archived = !budget.active;
                card.value = Some(budget.limit);
                card.currency = Some(budget.currency.clone());
            }),
        EntityKind::Automation | EntityKind::Workflow => doc
            .workflows
            .iter()
            .find(|workflow| workflow.id == id)
            .map(|workflow| {
                card.title = workflow.name.clone();
                card.archived = !workflow.active;
            }),
        EntityKind::Reminder => {
            doc.reminders
                .iter()
                .find(|reminder| reminder.id == id)
                .map(|reminder| {
                    card.title = reminder.text.clone();
                    card.archived = reminder.done || reminder.muted;
                })
        }
        EntityKind::Alert => doc
            .price_alerts
            .iter()
            .find(|alert| alert.id == id)
            .map(|alert| {
                card.title = alert.asset.symbol.clone();
                card.archived = !alert.active;
                card.value = Some(alert.price);
                card.currency = Some(alert.currency.clone());
            }),
        EntityKind::Asset => doc
            .holdings
            .iter()
            .map(|holding| &holding.asset)
            .chain(doc.price_alerts.iter().map(|alert| &alert.asset))
            .find(|asset| asset.key() == id)
            .map(|asset| card.title = asset.symbol.clone()),
        EntityKind::Scenario => find_calc_scenario(doc, id).map(|scenario| {
            card.title = scenario.name.clone();
            card.value = mwt_base::calc::invest::simulate(&scenario.input)
                .ok()
                .map(|result| result.final_nominal);
            card.currency = Some(scenario.input.currency.clone());
        }),
    };
    card.missing = found.is_none();
    card
}

/// Everything linked to `target`, grouped; empty groups are left out.
pub fn relations(doc: &VaultDoc, target: &EntityRef) -> Vec<RelationGroup> {
    let id = target.id.as_str();
    let groups = match target.kind {
        EntityKind::Store => store_relations(doc, id),
        EntityKind::Tx => tx_relations(doc, id),
        EntityKind::Category => category_relations(doc, id),
        EntityKind::Contact => contact_relations(doc, id),
        EntityKind::Goal => goal_relations(doc, id),
        EntityKind::Budget => budget_relations(doc, id),
        EntityKind::Automation | EntityKind::Workflow => workflow_relations(doc, id),
        EntityKind::Reminder => reminder_relations(doc, id),
        EntityKind::Alert => alert_relations(doc, id),
        EntityKind::Asset => asset_relations(doc, id),
        EntityKind::Scenario => scenario_relations(doc, id),
    };
    groups.into_iter().flatten().collect()
}

fn tx_by_id<'doc>(doc: &'doc VaultDoc, id: &str) -> Option<&'doc Transaction> {
    doc.transactions.iter().find(|tx| tx.meta().id == id)
}

fn store_item(doc: &VaultDoc, id: &str) -> Option<RelationItem> {
    find_store(doc, id).map(|store| {
        let mut item = RelationItem::named(EntityKind::Store, &store.id, &store.name);
        item.archived = store.archived;
        item
    })
}

/// Workflows split by whether they are automations (built by the form).
pub fn workflows_where(
    doc: &VaultDoc,
    used: impl Fn(&Workflow) -> bool,
) -> [Option<RelationGroup>; 2] {
    let (automations, workflows): (Vec<_>, Vec<_>) = doc
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
fn scope_covers(doc: &VaultDoc, scope: &StoreScope, store_id: &str) -> bool {
    !scope.store_ids.is_empty() && resolve_scope(doc, scope).contains(store_id)
}

fn reminders_of(doc: &VaultDoc, id: &str) -> Option<RelationGroup> {
    let items = doc
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
            let mut item = RelationItem::named(EntityKind::Reminder, &reminder.id, &reminder.text);
            item.when = Some(reminder.due_at.clone());
            item.archived = reminder.done;
            item
        })
        .collect();
    group(RelationKind::Reminders, items)
}

fn store_relations(doc: &VaultDoc, id: &str) -> Vec<Option<RelationGroup>> {
    let Some(store) = find_store(doc, id) else {
        return Vec::new();
    };
    let parent = store
        .parent_id
        .as_deref()
        .and_then(|parent| store_item(doc, parent))
        .into_iter()
        .collect();
    let children = doc
        .stores
        .iter()
        .filter(|child| child.parent_id.as_deref() == Some(id))
        .filter_map(|child| store_item(doc, &child.id))
        .collect();
    let assets = doc
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
    let goals = doc
        .goals
        .iter()
        .filter(|goal| scope_covers(doc, &goal.scope, id))
        .map(|goal| {
            let mut item = RelationItem::named(EntityKind::Goal, &goal.id, &goal.name);
            item.archived = !goal.active;
            item
        })
        .collect();
    let budgets = doc
        .budgets
        .iter()
        .filter(|budget| {
            matches!(&budget.target, BudgetTarget::Store(scope) if scope_covers(doc, scope, id))
        })
        .map(budget_item)
        .collect();
    let alerts = doc
        .price_alerts
        .iter()
        .filter(|alert| alert.store_id.as_deref() == Some(id))
        .map(alert_item)
        .collect();
    let txs = newest_first(
        doc.transactions
            .iter()
            .filter(|tx| tx.touches(id))
            .collect(),
    );
    let [automations, workflows] = workflows_where(doc, |workflow| quoted_in(workflow, id));
    vec![
        group(RelationKind::Parent, parent),
        group(RelationKind::Children, children),
        group(RelationKind::Assets, assets),
        group(RelationKind::Goals, goals),
        group(RelationKind::Budgets, budgets),
        automations,
        workflows,
        reminders_of(doc, id),
        group(RelationKind::Alerts, alerts),
        group(RelationKind::Transactions, txs),
    ]
}

fn tx_relations(doc: &VaultDoc, id: &str) -> Vec<Option<RelationGroup>> {
    let Some(tx) = tx_by_id(doc, id) else {
        return Vec::new();
    };
    let origin_id = match &tx.meta().origin {
        TxOrigin::Automation { automation_id, .. } => Some(automation_id.as_str()),
        TxOrigin::Workflow { workflow_id, .. } => Some(workflow_id.as_str()),
        TxOrigin::Manual | TxOrigin::Import { .. } => None,
    };
    let origin = origin_id
        .and_then(|origin| doc.workflows.iter().find(|workflow| workflow.id == origin))
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
            doc.transactions
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
        .filter_map(|store| store_item(doc, store))
        .collect();
    let categories = tx
        .meta()
        .categories
        .iter()
        .filter_map(|key| category_item(doc, key))
        .collect();
    let mut seen = HashSet::new();
    let contacts = tx
        .contact_ids()
        .into_iter()
        .filter(|contact| seen.insert(*contact))
        .filter_map(|contact| contact_item(doc, contact))
        .collect();
    vec![
        group(RelationKind::Origin, origin),
        group(RelationKind::Stores, stores),
        group(RelationKind::Categories, categories),
        group(RelationKind::Contacts, contacts),
        group(RelationKind::Flow, flow),
    ]
}

fn category_item(doc: &VaultDoc, key: &str) -> Option<RelationItem> {
    doc.categories
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

fn contact_item(doc: &VaultDoc, id: &str) -> Option<RelationItem> {
    doc.contacts
        .iter()
        .find(|contact| contact.id == id)
        .map(|contact| {
            let mut item = RelationItem::named(EntityKind::Contact, &contact.id, &contact.name);
            item.archived = contact.archived;
            item
        })
}

fn category_relations(doc: &VaultDoc, key: &str) -> Vec<Option<RelationGroup>> {
    let budgets = doc
        .budgets
        .iter()
        .filter(|budget| matches!(&budget.target, BudgetTarget::Category(used) if used == key))
        .map(budget_item)
        .collect();
    let mut rules: Vec<_> = doc
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
        doc.transactions
            .iter()
            .filter(|tx| tx.meta().categories.iter().any(|used| used == key))
            .collect(),
    );
    let [automations, workflows] = workflows_where(doc, |workflow| quoted_in(workflow, key));
    vec![
        group(RelationKind::Budgets, budgets),
        group(RelationKind::Rules, rules),
        automations,
        workflows,
        group(RelationKind::Transactions, txs),
    ]
}

fn contact_relations(doc: &VaultDoc, id: &str) -> Vec<Option<RelationGroup>> {
    let stores = doc
        .stores
        .iter()
        .filter(|store| store.contact_id.as_deref() == Some(id))
        .filter_map(|store| store_item(doc, &store.id))
        .collect();
    let txs = newest_first(
        doc.transactions
            .iter()
            .filter(|tx| tx.contact_ids().contains(&id))
            .collect(),
    );
    let [automations, workflows] = workflows_where(doc, |workflow| quoted_in(workflow, id));
    vec![
        group(RelationKind::Stores, stores),
        automations,
        workflows,
        reminders_of(doc, id),
        group(RelationKind::Transactions, txs),
    ]
}

fn goal_relations(doc: &VaultDoc, id: &str) -> Vec<Option<RelationGroup>> {
    let Some(goal) = doc.goals.iter().find(|goal| goal.id == id) else {
        return Vec::new();
    };
    let stores = goal
        .scope
        .store_ids
        .iter()
        .filter_map(|store| store_item(doc, store))
        .collect();
    let scenario = goal
        .scenario_id
        .as_deref()
        .and_then(|scenario| find_calc_scenario(doc, scenario))
        .map(|scenario| RelationItem::named(EntityKind::Scenario, &scenario.id, &scenario.name));
    // What fills the goal: automations sending money to its wallets.
    let [automations, workflows] = workflows_where(doc, |workflow| {
        goal.scope
            .store_ids
            .iter()
            .any(|store| quoted_in(workflow, store))
    });
    vec![
        group(RelationKind::Stores, stores),
        automations,
        workflows,
        group(RelationKind::Scenarios, scenario.into_iter().collect()),
        reminders_of(doc, id),
    ]
}

fn scenario_relations(doc: &VaultDoc, id: &str) -> Vec<Option<RelationGroup>> {
    let Some(scenario) = find_calc_scenario(doc, id) else {
        return Vec::new();
    };
    let goals = doc
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
        .filter_map(|store| store_item(doc, store))
        .collect();
    vec![
        group(RelationKind::Goals, goals),
        group(RelationKind::Stores, stores),
    ]
}

fn budget_relations(doc: &VaultDoc, id: &str) -> Vec<Option<RelationGroup>> {
    let Some(budget) = doc.budgets.iter().find(|budget| budget.id == id) else {
        return Vec::new();
    };
    match &budget.target {
        BudgetTarget::Total => Vec::new(),
        BudgetTarget::Category(key) => vec![group(
            RelationKind::Categories,
            category_item(doc, key).into_iter().collect(),
        )],
        BudgetTarget::Store(scope) => vec![group(
            RelationKind::Stores,
            scope
                .store_ids
                .iter()
                .filter_map(|store| store_item(doc, store))
                .collect(),
        )],
    }
}

fn workflow_relations(doc: &VaultDoc, id: &str) -> Vec<Option<RelationGroup>> {
    let Some(workflow) = doc.workflows.iter().find(|workflow| workflow.id == id) else {
        return Vec::new();
    };
    let created = newest_first(
        doc.transactions
            .iter()
            .filter(|tx| made_by(tx, id))
            .collect(),
    );
    let stores = doc
        .stores
        .iter()
        .filter(|store| quoted_in(workflow, &store.id))
        .filter_map(|store| store_item(doc, &store.id))
        .collect();
    let categories = doc
        .categories
        .iter()
        .filter(|category| quoted_in(workflow, &category.key))
        .filter_map(|category| category_item(doc, &category.key))
        .collect();
    vec![
        group(RelationKind::Stores, stores),
        group(RelationKind::Categories, categories),
        group(RelationKind::Created, created),
    ]
}

fn reminder_relations(doc: &VaultDoc, id: &str) -> Vec<Option<RelationGroup>> {
    let Some(source) = doc
        .reminders
        .iter()
        .find(|reminder| reminder.id == id)
        .and_then(|reminder| reminder.source.as_ref())
    else {
        return Vec::new();
    };
    let source_id = source.id.as_str();
    let stores = store_item(doc, source_id).into_iter().collect();
    let goals = doc
        .goals
        .iter()
        .filter(|goal| goal.id == source_id)
        .map(|goal| RelationItem::named(EntityKind::Goal, &goal.id, &goal.name))
        .collect();
    let contacts = contact_item(doc, source_id).into_iter().collect();
    vec![
        group(RelationKind::Stores, stores),
        group(RelationKind::Goals, goals),
        group(RelationKind::Contacts, contacts),
    ]
}

fn alert_relations(doc: &VaultDoc, id: &str) -> Vec<Option<RelationGroup>> {
    let Some(alert) = doc.price_alerts.iter().find(|alert| alert.id == id) else {
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
        .and_then(|store| store_item(doc, store))
        .into_iter()
        .collect();
    vec![
        group(RelationKind::Assets, asset),
        group(RelationKind::Stores, stores),
    ]
}

fn asset_relations(doc: &VaultDoc, key: &str) -> Vec<Option<RelationGroup>> {
    let stores = doc
        .holdings
        .iter()
        .filter(|holding| holding.asset.key() == key && !holding.quantity.is_zero())
        .filter_map(|holding| {
            store_item(doc, &holding.store_id).map(|mut item| {
                item.value = Some(holding.quantity);
                item.currency = Some(holding.asset.symbol.clone());
                item
            })
        })
        .collect();
    let alerts = doc
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

fn budget_item(budget: &Budget) -> RelationItem {
    let mut item = RelationItem::named(EntityKind::Budget, &budget.id, "");
    item.value = Some(budget.limit);
    item.currency = Some(budget.currency.clone());
    item.archived = !budget.active;
    item
}

fn alert_item(alert: &PriceAlert) -> RelationItem {
    let mut item = RelationItem::named(EntityKind::Alert, &alert.id, &alert.asset.symbol);
    item.value = Some(alert.price);
    item.currency = Some(alert.currency.clone());
    item.archived = !alert.active;
    item
}

#[cfg(test)]
mod tests {
    use mwt_model::TxOrigin;
    use mwt_testkit::builders::topup;

    use super::*;

    fn workflow(id: &str) -> Workflow {
        Workflow {
            id: id.into(),
            name: "Salary".into(),
            description: String::new(),
            active: true,
            graph: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            react_to_workflows: false,
            max_amount: None,
            max_amount_currency: None,
            versions: Vec::new(),
            tests: Vec::new(),
            draft: None,
            automation: None,
        }
    }

    fn made(workflow_id: &str, income_ref: Option<&str>) -> Transaction {
        let mut tx = topup("card", "100");
        tx.meta_mut().origin = TxOrigin::Workflow {
            workflow_id: workflow_id.into(),
            run_id: String::new(),
            planned_at: None,
            planned_date: None,
            estimated: false,
            run_key: None,
        };
        tx.meta_mut().income_ref = income_ref.map(str::to_string);
        tx
    }

    #[test]
    fn a_distributing_salary_run_lists_the_income_and_not_its_transfers() {
        let mut doc = VaultDoc::default();
        doc.workflows.push(workflow("salary"));
        let income = made("salary", None);
        let income_id = income.meta().id.clone();
        let transfer = made("salary", Some(&income_id));
        doc.transactions.push(income);
        doc.transactions.push(transfer);
        let groups = relations(&doc, &EntityRef::new(EntityKind::Workflow, "salary"));
        let created = groups
            .iter()
            .find(|group| group.kind == RelationKind::Created)
            .expect("a created group");
        assert_eq!(created.total, 1);
        assert_eq!(created.items[0].target.id, income_id);
    }
}
