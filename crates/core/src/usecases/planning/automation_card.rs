//! Scenarios of the automation card and the journal: the card of a scheme or
//! a repeat, and every run and notice in one list. Reading a scheme's graph
//! (its trigger and actions) is here because `planning` may not use
//! `workflow`; the facts that need only the document are built in
//! `planning::automation_card`.

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::error::{Error, Result};
use crate::model::planning::{TransactionDraft, WorkdayShift};
use crate::model::workflow::{Workflow, WorkflowRun};
use crate::planning::automation_card::{
    self as rules, AutomationCard, CARD_RUNS, CardAction, CardMoney, CardTrigger, JournalEntry,
    first_store, schedule_of, store_of, text, workday_of,
};
use crate::planning::my_automations::{MineKind, lookahead_until};
use crate::service::UnlockedVault;
use crate::workflow::graph::{Graph, Node};
use crate::workflow::nodes::triggers::schedule_moments_limited;
use crate::workflow::types::WfValue;

fn trigger_of(node: &Node) -> CardTrigger {
    match node.kind.as_str() {
        "trigger.schedule" => CardTrigger::Schedule,
        "trigger.manual" => CardTrigger::Manual,
        "trigger.transaction" => CardTrigger::Transaction {
            tx_kind: text(&node.params, "tx_kind").unwrap_or_else(|| "any".into()),
            category: text(&node.params, "category"),
            store: first_store(&node.params),
        },
        other => CardTrigger::Other {
            node_kind: other.to_string(),
        },
    }
}

/// The node an input port is wired from, with its port.
fn source<'a>(graph: &'a Graph, node: &str, port: &str) -> Option<(&'a Node, &'a str)> {
    let edge = graph
        .edges
        .iter()
        .find(|edge| edge.to == node && edge.to_port == port)?;
    let from = graph
        .nodes
        .iter()
        .find(|candidate| candidate.id == edge.from)?;
    Some((from, edge.from_port.as_str()))
}

/// The share a "Math" node set to percent takes.
fn percent_of(graph: &Graph, node: &Node) -> Option<(Amount, String)> {
    let (math, _) = source(graph, &node.id, "amount")?;
    if math.kind != "logic.math" || text(&math.params, "op").as_deref() != Some("percent") {
        return None;
    }
    Some((
        Amount::parse(&text(&math.params, "b")?).ok()?,
        math.id.clone(),
    ))
}

fn action_of(graph: &Graph, node: &Node) -> CardAction {
    let params = &node.params;
    let mut action = CardAction {
        kind: node.kind.clone(),
        percent: None,
        money: CardMoney::default(),
        title: String::new(),
    };
    match node.kind.as_str() {
        "action.transaction" => {
            action.kind = text(params, "tx_kind").unwrap_or_else(|| "transfer".into());
            action.percent = percent_of(graph, node).map(|(share, _)| share);
            action.money.amount = text(params, "amount").and_then(|raw| Amount::parse(&raw).ok());
            action.money.currency = text(params, "asset").map(|code| code.to_lowercase());
            action.money.to_store = store_of(params.get("to"));
            action.money.from_store = store_of(params.get("account")).or_else(|| {
                match source(graph, &node.id, "account") {
                    Some((from, _)) if from.kind == "trigger.transaction" => {
                        first_store(&from.params)
                    }
                    _ => None,
                }
            });
            action.money.category = text(params, "category");
            if let Some(draft) = text(params, "template")
                .and_then(|raw| serde_json::from_str::<TransactionDraft>(&raw).ok())
            {
                action.money.fill_from(&draft);
            }
        }
        "action.distribute" => action.money.from_store = store_of(params.get("from")),
        "action.notify" | "action.reminder" => {
            action.title = text(params, "title").unwrap_or_default()
        }
        _ => {}
    }
    if action.kind == "action.distribute" {
        action.kind = "distribute".into();
    } else if let Some(short) = action.kind.strip_prefix("action.") {
        action.kind = short.to_string();
    }
    action
}

/// The money a sleeping run will move, from what it computed before it slept.
fn asked_money(graph: &Graph, memo: &str) -> CardMoney {
    let saved: Vec<(String, String, WfValue)> = serde_json::from_str(memo).unwrap_or_default();
    let value = |node: &str, port: &str| {
        saved
            .iter()
            .find(|(from, out, _)| from == node && out == port)
            .map(|(_, _, value)| value.clone())
    };
    let Some(node) = graph
        .nodes
        .iter()
        .find(|node| !node.disabled && node.kind == "action.transaction")
    else {
        return CardMoney::default();
    };
    let mut money = action_of(graph, node).money;
    let wired = |port: &str| {
        source(graph, &node.id, port).and_then(|(from, from_port)| value(&from.id, from_port))
    };
    let amount = wired("amount").or_else(|| {
        // The share is counted after the answer: take it of the income.
        let (share, math) = percent_of(graph, node)?;
        let (income, port) = source(graph, &math, "a")?;
        match value(&income.id, port)? {
            WfValue::Money { amount, currency } => Some(WfValue::Money {
                amount: amount * share / Amount::from_f64(100.0),
                currency,
            }),
            _ => None,
        }
    });
    match amount {
        Some(WfValue::Money { amount, currency }) => {
            money.amount = Some(amount.abs());
            money.currency = Some(currency);
        }
        Some(WfValue::Number(amount)) => money.amount = Some(amount.abs()),
        _ => {}
    }
    if let Some(WfValue::Account(account)) = wired("account") {
        money.from_store = account.store_id().map(str::to_string).or(money.from_store);
    }
    if let Some(WfValue::Account(account)) = wired("to") {
        money.to_store = account.store_id().map(str::to_string).or(money.to_store);
    }
    money
}

/// One run as a journal row; a sleeping run's money is read from its graph.
fn journal_entry(vault: &UnlockedVault, workflow: &Workflow, run: &WorkflowRun) -> JournalEntry {
    rules::journal_entry(&vault.doc, workflow, run, |source, memo| {
        Graph::parse(source)
            .map(|graph| asked_money(&graph, memo))
            .unwrap_or_default()
    })
}

/// Every run and every reminder or alert shown, the newest first.
pub fn automation_journal(vault: &UnlockedVault, limit: usize) -> Vec<JournalEntry> {
    let mut out: Vec<JournalEntry> = vault
        .doc
        .workflow_runs
        .iter()
        .filter(|run| !run.dry_run && !rules::answered_question(&vault.doc, run))
        .filter_map(|run| {
            let workflow = vault
                .doc
                .workflows
                .iter()
                .find(|workflow| workflow.id == run.workflow_id)?;
            Some(journal_entry(vault, workflow, run))
        })
        .collect();
    out.extend(rules::notice_entries(
        crate::planning::notifications::notifications(&vault.doc),
    ));
    rules::newest(out, limit)
}

/// The card of a scheme or a repeat (both are workflows).
pub fn automation_card(
    vault: &UnlockedVault,
    id: &str,
    now: i64,
    zone: &str,
) -> Result<AutomationCard> {
    let workflow = vault
        .doc
        .workflows
        .iter()
        .find(|workflow| workflow.id == id)
        .ok_or_else(|| Error::Missing(format!("workflow {id}")))?;
    let cal = Calendar::new(zone);
    let until = lookahead_until(now);
    let runs = vault
        .doc
        .workflow_runs
        .iter()
        .rev()
        .filter(|run| {
            run.workflow_id == id && !run.dry_run && !rules::answered_question(&vault.doc, run)
        })
        .take(CARD_RUNS)
        .map(|run| journal_entry(vault, workflow, run))
        .collect();
    let mut card = AutomationCard {
        kind: MineKind::Scheme,
        id: workflow.id.clone(),
        name: workflow.name.clone(),
        description: workflow.description.clone(),
        active: workflow.active,
        schedule: None,
        workday: WorkdayShift::None,
        follows: None,
        trigger: CardTrigger::Manual,
        actions: Vec::new(),
        asks: false,
        next_at: None,
        runs,
    };
    if let Some(automation) = crate::usecases::planning::automations::automations(vault)
        .into_iter()
        .find(|automation| automation.id == id)
    {
        let next = vault
            .automation_occurrences(&automation, &cal, now, until)
            .into_iter()
            .next();
        rules::fill_repeat(&mut card, automation, next);
        return Ok(card);
    }
    let graph = Graph::parse(&workflow.graph)?;
    let live: Vec<&Node> = graph.nodes.iter().filter(|node| !node.disabled).collect();
    if let Some(trigger) = live.iter().find(|node| node.kind.starts_with("trigger.")) {
        card.trigger = trigger_of(trigger);
        card.schedule = schedule_of(&trigger.params);
        card.workday = workday_of(&trigger.params);
    }
    card.next_at = live
        .iter()
        .filter(|node| node.kind == "trigger.schedule")
        .filter_map(|node| {
            schedule_moments_limited(&node.params, &cal, now, until, 1)
                .0
                .first()
                .copied()
        })
        .min()
        .map(calendar::iso);
    if let CardTrigger::Transaction {
        category: Some(category),
        ..
    } = card.trigger.clone()
    {
        follow_income(vault, &mut card, &category, &cal, now, until);
    }
    card.asks = live.iter().any(|node| node.kind == "action.confirm");
    card.actions = live
        .iter()
        .filter(|node| node.kind.starts_with("action.") && node.kind != "action.confirm")
        .map(|node| action_of(&graph, node))
        .collect();
    Ok(card)
}

/// A scheme on an income fires when that income comes: "When" and
/// "Next time" follow the soonest repeat that books its category.
fn follow_income(
    vault: &UnlockedVault,
    card: &mut AutomationCard,
    category: &str,
    cal: &Calendar,
    now: i64,
    until: i64,
) {
    let soonest = crate::usecases::planning::automations::automations(vault)
        .into_iter()
        .filter(|automation| automation.active)
        .filter_map(|automation| {
            let (moment, tx) = vault
                .automation_occurrences(&automation, cal, now, until)
                .into_iter()
                .next()?;
            tx.meta()
                .categories
                .iter()
                .any(|key| key == category)
                .then_some((moment, automation))
        })
        .min_by_key(|(moment, _)| *moment);
    if let Some((moment, automation)) = soonest {
        rules::follow(card, moment, automation);
    }
}
