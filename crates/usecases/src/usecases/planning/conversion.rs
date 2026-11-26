//! op-period T24 (M5, M6, Q14): the one-time conversion on open of the old «salary -> deposit»
//! automation into row 1 of an income template's distribution rule. Strict: exactly one fitting
//! pair converts, anything else changes nothing and says nothing (only the marker is written). Old income + manual transfer
//! pairs are never linked (M5).

use crate::Amount;
use crate::error::Result;
use crate::model::planning::{AmountMode, AutomationPreset, TransactionDraft};
use crate::model::store::{Binding, Store, SubType};
use crate::model::vault::{VaultDoc, now_iso};
use crate::planning::templates::{TemplateDraft, plan_create_template};
use crate::service::Session;
use crate::workflow::graph::{Graph, Node};
use mwt_model::{
    AccountRef, DistKind, DistTarget, DistributionRow, DistributionRule, OnOtherAmount,
    OperationTemplate, TemplateKind, Workflow,
};

/// The marker in `Settings.migrations_done` of a vault this conversion has run on.
pub const SALARY_RULE_MARKER: &str = "op-period-salary-rule";

/// The workflow node that moves money by shares or sums.
const DISTRIBUTE_NODE: &str = "action.distribute";
/// Every node that does something to the vault is of a kind that starts so.
const ACTION_PREFIX: &str = "action.";

/// A salary automation and the transfer automation that moves its money on to a deposit.
struct Pair {
    income_id: String,
    transfer_id: String,
    template: OperationTemplate,
}

/// The id of the template converted from the income automation `income_id`: derived from the
/// conversion marker and that id, so two devices that convert one vault before they sync write
/// the same record and the two merge into one (M6, T2).
pub(crate) fn template_id_for(income_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{SALARY_RULE_MARKER}:{income_id}"));
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    uuid::Builder::from_custom_bytes(bytes)
        .into_uuid()
        .to_string()
}

/// Record that the conversion has had its one pass over `doc`.
pub(crate) fn mark_done(doc: &mut VaultDoc) {
    doc.settings
        .migrations_done
        .push(SALARY_RULE_MARKER.to_string());
}

/// Convert the vault's salary -> deposit pair if there is exactly one that fits. The marker is
/// written on the first pass whatever the outcome, so the conversion runs once per vault (M6): a
/// pair made later is never converted. Returns whether the vault changed.
pub fn convert_salary_to_rule(vault: &mut Session) -> Result<bool> {
    if vault
        .doc
        .settings
        .migrations_done
        .iter()
        .any(|done| done == SALARY_RULE_MARKER)
    {
        return Ok(false);
    }
    let pair = find_pair(&vault.doc);
    vault.transact(|tx| {
        if let Some(pair) = &pair {
            tx.collection::<OperationTemplate>()
                .insert(pair.template.clone())?;
            tx.collection::<Workflow>()
                .update(&pair.income_id, |workflow| {
                    if let Some(spec) = workflow.automation.as_mut() {
                        spec.operation_template_id = Some(pair.template.id.clone());
                    }
                })?;
            tx.collection::<Workflow>()
                .update(&pair.transfer_id, |workflow| workflow.active = false)?;
        }
        mark_done(tx.doc_mut());
        Ok(())
    })?;
    Ok(true)
}

/// The one pair that converts, or `None`: no pair, two pairs, or one that does not fit.
fn find_pair(doc: &VaultDoc) -> Option<Pair> {
    let incomes = salary_incomes(doc);
    let mut candidates = Vec::new();
    for (income, income_store) in &incomes {
        for workflow in doc.workflows.iter().filter(|workflow| workflow.active) {
            if moves_to_deposit(doc, workflow, income_store) {
                candidates.push((*income, workflow));
            }
        }
    }
    let [(income, transfer)] = candidates.as_slice() else {
        return None;
    };
    fitting_template(doc, income, transfer)
}

/// Active `Salary` automations with the store each one pays into.
fn salary_incomes(doc: &VaultDoc) -> Vec<(&Workflow, String)> {
    doc.workflows
        .iter()
        .filter(|workflow| workflow.active)
        .filter_map(|workflow| {
            let spec = workflow.automation.as_ref()?;
            if spec.preset != Some(AutomationPreset::Salary) {
                return None;
            }
            match &spec.template {
                TransactionDraft::Topup(topup) => Some((workflow, topup.store_id.clone())),
                _ => None,
            }
        })
        .collect()
}

/// Whether an active workflow moves money out of `from_store` to a deposit or savings store of
/// the user: a transfer automation, or a graph with an `action.distribute` node.
fn moves_to_deposit(doc: &VaultDoc, workflow: &Workflow, from_store: &str) -> bool {
    if let Some(spec) = &workflow.automation {
        return match &spec.template {
            TransactionDraft::Transfer(transfer) if is_store(&transfer.from, from_store) => {
                deposit_of(doc, &transfer.to).is_some()
            }
            _ => false,
        };
    }
    distribute_node(doc, workflow, from_store).is_some()
}

/// The one enabled action of a graph, when it is an `action.distribute` that pays out of
/// `from_store` to a deposit: a graph with any other enabled action does more than the rule moves.
fn distribute_node(doc: &VaultDoc, workflow: &Workflow, from_store: &str) -> Option<Node> {
    let graph = Graph::parse(&workflow.graph).ok()?;
    let mut actions = graph
        .nodes
        .into_iter()
        .filter(|node| node.kind.starts_with(ACTION_PREFIX) && !node.disabled);
    let only = actions.next().filter(|_| actions.next().is_none())?;
    Some(only)
        .filter(|node| node.kind == DISTRIBUTE_NODE)
        .filter(|node| {
            node.params
                .get("from")
                .and_then(account_param)
                .is_some_and(|from| is_store(&from, from_store))
        })
        .filter(|node| {
            targets_of(node).iter().any(|item| {
                target_account(item).is_some_and(|account| deposit_of(doc, &account).is_some())
            })
        })
}

fn targets_of(node: &Node) -> Vec<serde_json::Value> {
    node.params
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn target_account(item: &serde_json::Value) -> Option<AccountRef> {
    account_param(item.get("account")?)
}

fn account_param(value: &serde_json::Value) -> Option<AccountRef> {
    match value {
        serde_json::Value::String(id) if !id.trim().is_empty() => {
            Some(AccountRef::Store(id.trim().to_string()))
        }
        serde_json::Value::Object(_) => serde_json::from_value(value.clone()).ok(),
        _ => None,
    }
}

fn is_store(account: &AccountRef, id: &str) -> bool {
    matches!(account, AccountRef::Store(store_id) if store_id == id)
}

/// The id of the user's own deposit or savings store an account points to.
fn deposit_of(doc: &VaultDoc, account: &AccountRef) -> Option<String> {
    let AccountRef::Store(id) = account else {
        return None;
    };
    let store: &Store = doc.stores.iter().find(|store| store.id == *id)?;
    let is_deposit = matches!(store.sub_type, Some(SubType::Deposit | SubType::Savings));
    (is_deposit && !store.archived && store.contact_id.is_none()).then(|| store.id.clone())
}

/// What the old transfer moves: a fixed sum of one currency and category to one deposit.
struct Movement {
    amount: Amount,
    target: String,
    asset_key: String,
    categories: Vec<String>,
}

/// The fixed single move of an old transfer automation or of its `action.distribute` node.
fn movement_of(doc: &VaultDoc, workflow: &Workflow, from_store: &str) -> Option<Movement> {
    if let Some(spec) = &workflow.automation {
        let TransactionDraft::Transfer(moved) = &spec.template else {
            return None;
        };
        return (spec.amount_mode == AmountMode::Fixed).then_some(Movement {
            amount: moved.amount,
            target: deposit_of(doc, &moved.to)?,
            asset_key: moved.asset.key(),
            categories: moved.meta.categories.clone(),
        });
    }
    let node = distribute_node(doc, workflow, from_store)?;
    if node.params.get("mode").and_then(serde_json::Value::as_str) != Some("fixed") {
        return None;
    }
    let [item] = targets_of(&node).try_into().ok()?;
    let target = deposit_of(doc, &target_account(&item)?)?;
    let raw = match item.get("share")? {
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::String(text) => text.trim().replace(',', "."),
        _ => return None,
    };
    // The sum is of the deposit's currency; the amount itself is wired in the graph.
    let asset_key = match &doc.stores.iter().find(|store| store.id == target)?.binding {
        Binding::Asset(asset) => asset.key(),
        _ => return None,
    };
    let categories = match node.params.get("category") {
        Some(serde_json::Value::String(key)) if !key.is_empty() => vec![key.clone()],
        _ => Vec::new(),
    };
    Some(Movement {
        amount: Amount::parse(&raw).ok()?,
        target,
        asset_key,
        categories,
    })
}

/// The new template when the pair fits (fixed amount of one currency and category, one target).
fn fitting_template(doc: &VaultDoc, income: &Workflow, transfer: &Workflow) -> Option<Pair> {
    let income_spec = income.automation.as_ref()?;
    let TransactionDraft::Topup(topup) = &income_spec.template else {
        return None;
    };
    let moved = movement_of(doc, transfer, &topup.store_id)?;
    let fits = income_spec.operation_template_id.is_none()
        && moved.asset_key == topup.asset.key()
        && moved.categories == topup.meta.categories
        && moved.amount.is_positive();
    if !fits {
        return None;
    }
    let draft = TemplateDraft {
        name: income.name.clone(),
        kind: TemplateKind::Income,
        store_id: Some(topup.store_id.clone()),
        asset: Some(topup.asset.clone()),
        amount: None,
        categories: topup.meta.categories.clone(),
        comment: String::new(),
        contact_id: None,
        period_rule: None,
        distribution: Some(DistributionRule {
            rows: vec![
                DistributionRow {
                    kind: DistKind::Sum {
                        amount: moved.amount,
                    },
                    target: DistTarget::Store { id: moved.target },
                },
                DistributionRow {
                    kind: DistKind::Remainder,
                    target: DistTarget::None,
                },
            ],
            on_other_amount: OnOtherAmount::ShrinkRemainder,
            keep_on_card_min: None,
        }),
    };
    let template =
        plan_create_template(doc, draft, template_id_for(&income.id), &now_iso()).ok()?;
    Some(Pair {
        income_id: income.id.clone(),
        transfer_id: transfer.id.clone(),
        template,
    })
}
