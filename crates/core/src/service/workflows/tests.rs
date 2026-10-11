use serde_json::{Value, json};

use super::*;
use crate::calendar::now_secs;
use crate::model::store::StoreKind;
use crate::model::transaction::TxOrigin;
use crate::service::test_support::*;
use crate::workflow::RunOutcome;
use crate::workflow::exec::StepStatus;

fn node(id: &str, kind: &str, params: Value) -> Value {
    json!({ "id": id, "kind": kind, "params": params })
}

fn wire(from: &str, from_port: &str, to: &str, to_port: &str) -> Value {
    json!({
        "id": format!("{from}.{from_port}>{to}.{to_port}"),
        "from": from, "from_port": from_port, "to": to, "to_port": to_port,
    })
}

fn graph(nodes: Vec<Value>, edges: Vec<Value>) -> String {
    json!({ "version": 1, "nodes": nodes, "edges": edges }).to_string()
}

fn draft(name: &str, graph: String) -> WorkflowDraft {
    WorkflowDraft {
        name: name.into(),
        description: String::new(),
        active: true,
        graph,
        react_to_workflows: false,
        max_amount: None,
    }
}

fn save(vault: &mut UnlockedVault, name: &str, graph: String) -> Workflow {
    vault
        .save_workflow(None, draft(name, graph), now_secs())
        .unwrap()
        .workflow
}

fn store_ref(id: &str) -> Value {
    json!({ "store": id })
}

/// A card and a savings account, 1000 USD on the card.
fn wallets() -> (tempfile::TempDir, UnlockedVault, String, String) {
    let (dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Cash, None);
    let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
    vault
        .add_transaction(income(&card, "1000", "opening"))
        .unwrap();
    (dir, vault, card, savings)
}

fn transfer_node(id: &str, from: &str, to: &str, amount: &str) -> Value {
    node(
        id,
        "action.transaction",
        json!({ "tx_kind": "transfer", "account": store_ref(from), "to": store_ref(to), "asset": "USD", "amount": amount }),
    )
}

/// «Salary comes → 10 % of it to savings».
fn tithe_graph(card: &str, savings: &str) -> String {
    graph(
        vec![
            node(
                "t",
                "trigger.transaction",
                json!({ "tx_kind": "topup", "category": "salary" }),
            ),
            node("pct", "logic.math", json!({ "op": "percent", "b": "10" })),
            node(
                "d",
                "action.distribute",
                json!({ "from": store_ref(card), "targets": [{ "account": store_ref(savings), "share": "100" }] }),
            ),
        ],
        vec![
            wire("t", "out", "d", "in"),
            wire("t", "amount", "pct", "a"),
            wire("pct", "result", "d", "amount"),
        ],
    )
}

fn later() -> i64 {
    now_secs() + 5
}

#[test]
fn a_salary_sends_ten_percent_to_savings_once() {
    let (_dir, mut vault, card, savings) = wallets();
    let workflow = save(&mut vault, "Tithe", tithe_graph(&card, &savings));
    // Operations from before the workflow was switched on do not fire it.
    assert!(
        vault
            .run_due_workflows(later(), "UTC")
            .unwrap()
            .runs
            .is_empty()
    );

    vault
        .add_transaction(income(&card, "5000", "salary"))
        .unwrap();
    let pass = vault.run_due_workflows(later(), "UTC").unwrap();
    assert_eq!(pass.runs.len(), 1, "{:?}", pass.runs);
    assert_eq!(
        pass.runs[0].outcome,
        RunOutcome::Done,
        "{:?}",
        pass.runs[0].trace
    );
    assert_eq!(pass.created.len(), 1);
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("500"));
    let made = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap();
    assert!(
        matches!(&made.meta().origin, TxOrigin::Workflow { workflow_id, .. } if *workflow_id == workflow.id)
    );

    // Its own transfer and the same salary do not fire it again.
    let again = vault.run_due_workflows(later(), "UTC").unwrap();
    assert!(again.runs.is_empty(), "{:?}", again.runs);
    assert_eq!(vault.workflow_runs(Some(&workflow.id), 10).len(), 1);
}

#[test]
fn after_confirmation_the_operation_waits_in_the_queue() {
    let (_dir, mut vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("c", "action.confirm", json!({})),
        transfer_node("tx", &card, &savings, "300"),
    ];
    let edges = vec![wire("t", "out", "c", "in"), wire("c", "out", "tx", "in")];
    let workflow = save(&mut vault, "Queue", graph(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(report.run.queued, 1, "{:?}", report.run.trace);
    assert!(
        report
            .run
            .trace
            .iter()
            .any(|step| step.status == StepStatus::Queued)
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);

    let queue = vault.workflow_queue();
    assert_eq!(queue.len(), 1);
    vault.confirm_queued(&queue[0].id).unwrap();
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("300"));
    assert!(vault.workflow_queue().is_empty());
}

#[test]
fn a_failed_action_rolls_the_run_back_unless_it_has_an_error_way() {
    let (_dir, mut vault, card, savings) = wallets();
    // 100 goes, then 5000 does not: nothing of the run is kept.
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("first", &card, &savings, "100"),
        transfer_node("tx", &card, &savings, "5000"),
    ];
    let edges = vec![
        wire("t", "out", "first", "in"),
        wire("first", "out", "tx", "in"),
    ];
    let workflow = save(&mut vault, "Too much", graph(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert!(
        matches!(report.run.outcome, RunOutcome::Failed { .. }),
        "{:?}",
        report.run.outcome
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
    assert_eq!(vault.workflow_runs(Some(&workflow.id), 5).len(), 1);

    // With an «Error» way the run goes on and tells why.
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("tx", &card, &savings, "5000"),
        node("why", "logic.text", json!({ "template": "Не вышло: {a}" })),
        node("n", "action.notify", json!({})),
    ];
    let edges = vec![
        wire("t", "out", "tx", "in"),
        wire("tx", "error", "n", "in"),
        wire("tx", "reason", "why", "a"),
        wire("why", "text", "n", "text"),
    ];
    let workflow = save(&mut vault, "Handled", graph(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    assert_eq!(report.notices.len(), 1);
    assert!(
        report.notices[0].body.starts_with("Не вышло: "),
        "{}",
        report.notices[0].body
    );
    assert!(
        vault
            .notifications()
            .iter()
            .any(|notice| notice.body.starts_with("Не вышло"))
    );
}

#[test]
fn an_emulation_shows_the_operations_and_keeps_nothing() {
    let (_dir, mut vault, card, savings) = wallets();
    vault
        .add_transaction(income(&card, "5000", "salary"))
        .unwrap();
    let before = vault.doc.transactions.len();
    let report = vault
        .dry_run_workflow(&tithe_graph(&card, &savings), None, None, later(), "UTC")
        .unwrap();
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    assert!(report.run.dry_run);
    assert_eq!(report.transactions.len(), 1);
    assert_eq!(vault.doc.transactions.len(), before);
    assert!(vault.doc.workflow_runs.is_empty());
    // The values that went along the wires are reported for the editor.
    assert_eq!(
        report
            .edge_values
            .get("pct.result>d.amount")
            .map(String::as_str),
        Some("500 USD")
    );
}

#[test]
fn a_schedule_fires_once_per_planned_moment() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node(
            "s",
            "trigger.schedule",
            json!({ "every": "day", "time": "09:00" }),
        ),
        node("n", "action.notify", json!({ "text": "Доброе утро" })),
    ];
    let workflow = save(
        &mut vault,
        "Morning",
        graph(nodes, vec![wire("s", "out", "n", "in")]),
    );
    let start = unix_secs("2026-03-01T10:00:00Z").unwrap();
    vault.wf_set_mark(&workflow.id, "schedule:s", "2026-03-01T10:00:00Z");
    let pass = vault.run_due_workflows(start + 3 * 86_400, "UTC").unwrap();
    assert_eq!(pass.runs.len(), 3);
    assert_eq!(pass.notices.len(), 3);
    assert!(
        vault
            .run_due_workflows(start + 3 * 86_400, "UTC")
            .unwrap()
            .runs
            .is_empty()
    );
}

#[test]
fn a_counter_lives_between_runs() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("add", "var.add", json!({ "name": "count", "delta": "1" })),
    ];
    let workflow = save(
        &mut vault,
        "Counter",
        graph(nodes, vec![wire("t", "out", "add", "in")]),
    );
    for _ in 0..3 {
        vault
            .run_workflow_now(&workflow.id, None, later(), "UTC")
            .unwrap();
    }
    let scope = VarScope::Workflow(workflow.id.clone());
    assert_eq!(
        vault
            .wf_var(&scope, "count")
            .and_then(|value| value.as_number()),
        Some(amount("3"))
    );
    vault.delete_workflow(&workflow.id).unwrap();
    assert!(vault.workflow_vars().is_empty());
}

#[test]
fn a_balance_threshold_fires_on_crossing_only() {
    let (_dir, mut vault, card, _savings) = wallets();
    let nodes = vec![
        node(
            "low",
            "trigger.balance",
            json!({ "store": store_ref(&card), "asset": "USD", "condition": "below", "value": "500" }),
        ),
        node("n", "action.notify", json!({ "text": "Мало денег" })),
    ];
    save(
        &mut vault,
        "Low",
        graph(nodes, vec![wire("low", "out", "n", "in")]),
    );
    assert!(
        vault
            .run_due_workflows(later(), "UTC")
            .unwrap()
            .runs
            .is_empty()
    );
    vault
        .add_transaction(payment(&card, "600", "rent"))
        .unwrap();
    assert_eq!(
        vault.run_due_workflows(later(), "UTC").unwrap().runs.len(),
        1
    );
    vault
        .add_transaction(payment(&card, "100", "food"))
        .unwrap();
    assert!(
        vault
            .run_due_workflows(later(), "UTC")
            .unwrap()
            .runs
            .is_empty()
    );
    vault
        .add_transaction(income(&card, "1000", "salary"))
        .unwrap();
    assert!(
        vault
            .run_due_workflows(later(), "UTC")
            .unwrap()
            .runs
            .is_empty()
    );
    vault
        .add_transaction(payment(&card, "1000", "rent"))
        .unwrap();
    assert_eq!(
        vault.run_due_workflows(later(), "UTC").unwrap().runs.len(),
        1
    );
}

#[test]
fn the_cap_stops_a_run_that_moves_too_much() {
    let (_dir, mut vault, card, savings) = wallets();
    quote_usd(&mut vault);
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("tx", &card, &savings, "300"),
    ];
    let workflow = vault
        .save_workflow(
            None,
            WorkflowDraft {
                max_amount: Some(amount("100")),
                ..draft("Capped", graph(nodes, vec![wire("t", "out", "tx", "in")]))
            },
            now_secs(),
        )
        .unwrap()
        .workflow;
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert!(
        matches!(&report.run.outcome, RunOutcome::Failed { message } if message.contains("cap")),
        "{:?}",
        report.run.outcome
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
}

#[test]
fn a_wait_sleeps_in_the_vault_and_goes_on_later() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node(
            "text",
            "logic.text",
            json!({ "template": "Прошло, запуск был {a}" }),
        ),
        node("w", "action.wait", json!({ "hours": "2" })),
        node("n", "action.notify", json!({})),
    ];
    let edges = vec![
        wire("t", "at", "text", "a"),
        wire("t", "out", "w", "in"),
        wire("w", "out", "n", "in"),
        wire("text", "text", "n", "text"),
    ];
    let workflow = save(&mut vault, "Later", graph(nodes, edges));
    let now = later();
    let report = vault
        .run_workflow_now(&workflow.id, None, now, "UTC")
        .unwrap();
    assert!(
        matches!(report.run.outcome, RunOutcome::Waiting { .. }),
        "{:?}",
        report.run.outcome
    );
    assert_eq!(vault.doc.workflow_waits.len(), 1);
    assert!(
        vault
            .run_due_workflows(now + 3600, "UTC")
            .unwrap()
            .runs
            .is_empty()
    );
    let pass = vault.run_due_workflows(now + 2 * 3600, "UTC").unwrap();
    assert_eq!(pass.runs.len(), 1);
    assert_eq!(pass.notices.len(), 1);
    // The trigger's moment, known before the wait, is still there after it.
    assert!(
        pass.notices[0].body.contains(&crate::calendar::iso(now)),
        "{}",
        pass.notices[0].body
    );
    assert!(vault.doc.workflow_waits.is_empty());
}

#[test]
fn an_event_runs_another_workflow() {
    let (_dir, mut vault) = empty_vault();
    let sender = vec![
        node("t", "trigger.manual", json!({})),
        node(
            "e",
            "action.emit",
            json!({ "name": "paid", "payload": "ипотека" }),
        ),
    ];
    let sender = save(
        &mut vault,
        "Sender",
        graph(sender, vec![wire("t", "out", "e", "in")]),
    );
    let listener = vec![
        node("on", "trigger.event", json!({ "name": "paid" })),
        node("n", "action.notify", json!({})),
    ];
    save(
        &mut vault,
        "Listener",
        graph(
            listener,
            vec![
                wire("on", "out", "n", "in"),
                wire("on", "payload", "n", "text"),
            ],
        ),
    );
    vault
        .run_workflow_now(&sender.id, None, later(), "UTC")
        .unwrap();
    assert!(
        vault
            .notifications()
            .iter()
            .any(|notice| notice.body == "ипотека")
    );
}

#[test]
fn a_graph_with_errors_is_not_switched_on_and_versions_are_kept() {
    let (_dir, mut vault) = empty_vault();
    let broken = graph(
        vec![node("n", "action.notify", json!({ "text": "x" }))],
        vec![],
    );
    let base = draft("Broken", broken);
    assert!(vault.save_workflow(None, base.clone(), now_secs()).is_err());
    let saved = vault
        .save_workflow(
            None,
            WorkflowDraft {
                active: false,
                ..base.clone()
            },
            now_secs(),
        )
        .unwrap();
    assert!(saved.issues.iter().any(|issue| issue.code == "no_trigger"));

    let fixed = graph(
        vec![
            node("t", "trigger.manual", json!({})),
            node("n", "action.notify", json!({ "text": "x" })),
        ],
        vec![wire("t", "out", "n", "in")],
    );
    let id = saved.workflow.id.clone();
    let updated = vault
        .save_workflow(
            Some(&id),
            WorkflowDraft {
                graph: fixed,
                ..base
            },
            now_secs(),
        )
        .unwrap();
    assert!(updated.workflow.active);
    assert_eq!(updated.workflow.versions.len(), 1);
    let restored = vault
        .restore_workflow_version(&id, &updated.workflow.versions[0].saved_at, now_secs())
        .unwrap();
    assert!(
        !restored.workflow.active,
        "a broken version comes back switched off"
    );
    assert_eq!(restored.workflow.versions.len(), 2);
}

#[test]
fn a_workflow_travels_as_a_file() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("n", "action.notify", json!({ "text": "x" })),
    ];
    let workflow = save(
        &mut vault,
        "Shared",
        graph(nodes, vec![wire("t", "out", "n", "in")]),
    );
    let file = vault.export_workflow(&workflow.id).unwrap();
    let imported = vault.import_workflow(&file, now_secs()).unwrap().workflow;
    assert_ne!(imported.id, workflow.id);
    assert!(!imported.active);
    assert_eq!(imported.name, "Shared");
    assert_eq!(
        Graph::parse(&imported.graph).unwrap(),
        Graph::parse(&workflow.graph).unwrap()
    );
    assert!(vault.import_workflow("{}", now_secs()).is_err());
}

#[test]
fn for_each_goes_through_the_accounts() {
    let (_dir, mut vault, _card, _savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("list", "list.accounts", json!({})),
        node("each", "list.for_each", json!({})),
        node("name", "list.field", json!({ "field": "name" })),
        node("count", "var.add", json!({ "name": "seen" })),
        node("last", "var.set", json!({ "name": "last" })),
    ];
    let edges = vec![
        wire("t", "out", "each", "in"),
        wire("list", "items", "each", "items"),
        wire("each", "each", "count", "in"),
        wire("count", "out", "last", "in"),
        wire("each", "item", "name", "record"),
        wire("name", "value", "last", "value"),
    ];
    let workflow = save(&mut vault, "Loop", graph(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    let scope = VarScope::Workflow(workflow.id.clone());
    let wallets = vault
        .doc
        .stores
        .iter()
        .filter(|store| store.parent_id.is_none() && !store.archived)
        .count();
    assert_eq!(
        vault
            .wf_var(&scope, "seen")
            .and_then(|value| value.as_number()),
        Some(Amount::new(rust_decimal::Decimal::from(wallets as u64)))
    );
    assert!(matches!(
        vault.wf_var(&scope, "last"),
        Some(WfValue::Text(_))
    ));
}

#[test]
fn a_workday_check_sends_the_salary_on_holidays_another_way() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node(
            "day",
            "data.workday",
            json!({ "date": "2026-03-08", "calendar": "ru" }),
        ),
        node("b", "logic.branch", json!({})),
        node(
            "yes",
            "var.set",
            json!({ "name": "said", "scope": "global" }),
        ),
        node(
            "no",
            "var.set",
            json!({ "name": "said", "scope": "global" }),
        ),
        node(
            "work",
            "data.constant",
            json!({ "value_type": "text", "text": "work" }),
        ),
        node(
            "rest",
            "data.constant",
            json!({ "value_type": "text", "text": "rest" }),
        ),
    ];
    let edges = vec![
        wire("t", "out", "b", "in"),
        wire("day", "workday", "b", "condition"),
        wire("b", "true", "yes", "in"),
        wire("b", "false", "no", "in"),
        wire("work", "value", "yes", "value"),
        wire("rest", "value", "no", "value"),
    ];
    let workflow = save(&mut vault, "Workday", graph(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    assert_eq!(
        vault.wf_var(&VarScope::Global, "said"),
        Some(WfValue::Text("rest".into()))
    );
    assert_eq!(
        report
            .edge_values
            .get("day.workday>b.condition")
            .map(String::as_str),
        Some("false")
    );
}
