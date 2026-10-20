use serde_json::{Value, json};

use super::*;
use crate::calendar::now_secs;
use crate::model::store::StoreKind;
use crate::model::transaction::TxOrigin;
use crate::service::test_support::*;
use crate::workflow::RunOutcome;

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

/// «Ask first → yes: 300 to savings; no: a notice».
fn ask_graph(card: &str, savings: &str, timeout_hours: &str) -> String {
    graph(
        vec![
            node("t", "trigger.manual", json!({})),
            node(
                "c",
                "action.confirm",
                json!({ "question": "Отложить 300?", "timeout_hours": timeout_hours }),
            ),
            transfer_node("tx", card, savings, "300"),
            node("n", "action.notify", json!({ "text": "Не отложили" })),
        ],
        vec![
            wire("t", "out", "c", "in"),
            wire("c", "true", "tx", "in"),
            wire("c", "false", "n", "in"),
        ],
    )
}

#[test]
fn a_question_pauses_the_run_until_the_answer() {
    let (_dir, mut vault, card, savings) = wallets();
    let workflow = save(&mut vault, "Ask", ask_graph(&card, &savings, "0"));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert!(
        matches!(&report.run.outcome, RunOutcome::Asking { question, until: None, .. } if question == "Отложить 300?"),
        "{:?}",
        report.run.outcome
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
    // Nothing goes on by itself, however long it waits.
    assert!(
        vault
            .run_due_workflows(later() + 400 * 24 * 3600, "UTC")
            .unwrap()
            .runs
            .is_empty()
    );
    let waits = vault.workflow_waits();
    assert_eq!(waits.len(), 1);
    assert_eq!(waits[0].kind, WaitKind::Confirm);

    let answered = vault
        .answer_workflow_wait(&waits[0].id, true, later(), "UTC")
        .unwrap();
    assert_eq!(answered.run.outcome, RunOutcome::Done);
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("300"));
    assert!(vault.workflow_waits().is_empty());
}

#[test]
fn answering_no_goes_the_no_way() {
    let (_dir, mut vault, card, savings) = wallets();
    let workflow = save(&mut vault, "Ask", ask_graph(&card, &savings, "0"));
    vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    let id = vault.workflow_waits()[0].id.clone();
    let report = vault
        .answer_workflow_wait(&id, false, later(), "UTC")
        .unwrap();
    assert_eq!(report.notices.len(), 1);
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
    assert!(
        vault
            .answer_workflow_wait(&id, true, later(), "UTC")
            .is_err()
    );
}

#[test]
fn an_unanswered_question_with_a_deadline_counts_as_no() {
    let (_dir, mut vault, card, savings) = wallets();
    let workflow = save(&mut vault, "Ask", ask_graph(&card, &savings, "2"));
    let now = later();
    vault
        .run_workflow_now(&workflow.id, None, now, "UTC")
        .unwrap();
    assert!(
        vault
            .run_due_workflows(now + 3600, "UTC")
            .unwrap()
            .runs
            .is_empty()
    );
    let pass = vault.run_due_workflows(now + 2 * 3600, "UTC").unwrap();
    assert_eq!(pass.runs.len(), 1);
    assert!(
        pass.notices
            .iter()
            .any(|notice| notice.body == "Не отложили")
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
}

#[test]
fn an_emulation_asks_and_then_follows_the_answer() {
    let (_dir, vault, card, savings) = wallets();
    let graph_json = ask_graph(&card, &savings, "0");
    let asked = vault
        .dry_run_workflow(&graph_json, None, None, BTreeMap::new(), later(), "UTC")
        .unwrap();
    assert!(
        matches!(&asked.run.outcome, RunOutcome::Asking { node_id, .. } if node_id == "c"),
        "{:?}",
        asked.run.outcome
    );
    assert!(asked.transactions.is_empty());

    let yes = BTreeMap::from([("c".to_string(), true)]);
    let report = vault
        .dry_run_workflow(&graph_json, None, None, yes, later(), "UTC")
        .unwrap();
    assert_eq!(report.run.outcome, RunOutcome::Done);
    assert_eq!(report.transactions.len(), 1);

    let no = BTreeMap::from([("c".to_string(), false)]);
    let report = vault
        .dry_run_workflow(&graph_json, None, None, no, later(), "UTC")
        .unwrap();
    assert!(report.transactions.is_empty());
    assert_eq!(report.notices.len(), 1);
    // Emulation keeps nothing asleep.
    assert!(vault.workflow_waits().is_empty());
}

#[test]
fn an_old_confirm_wire_from_next_becomes_yes() {
    let (_dir, mut vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("c", "action.confirm", json!({})),
        transfer_node("tx", &card, &savings, "300"),
    ];
    let edges = vec![wire("t", "out", "c", "in"), wire("c", "out", "tx", "in")];
    let old = graph(nodes, edges);
    let workflow = save(&mut vault, "Old", old.clone());
    assert!(
        workflow.graph.contains("\"from_port\":\"true\""),
        "{}",
        workflow.graph
    );
    // A vault written by the older app is brought up to date when it opens.
    vault.doc.workflows[0].graph = old;
    assert!(vault.migrate_workflow_graphs());
    assert!(!vault.migrate_workflow_graphs());
    let parsed = Graph::parse(&vault.doc.workflows[0].graph).unwrap();
    assert_eq!(parsed.version, crate::workflow::graph::GRAPH_VERSION);
    assert!(
        parsed
            .edges
            .iter()
            .any(|edge| edge.from == "c" && edge.from_port == "true")
    );
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
        .dry_run_workflow(
            &tithe_graph(&card, &savings),
            None,
            None,
            BTreeMap::new(),
            later(),
            "UTC",
        )
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

#[test]
fn a_called_workflow_answers_and_shares_the_run() {
    let (_dir, mut vault, card, savings) = wallets();
    // The helper: moves what it is given to savings and answers «moved».
    let helper = vec![
        node("called", "trigger.called", json!({})),
        node(
            "move",
            "action.transaction",
            json!({ "tx_kind": "transfer", "account": store_ref(&card), "to": store_ref(&savings), "asset": "USD" }),
        ),
        node(
            "answer",
            "data.constant",
            json!({ "value_type": "text", "text": "moved" }),
        ),
        node("back", "action.return", json!({})),
    ];
    let helper = vault
        .save_workflow(
            None,
            WorkflowDraft {
                active: false,
                ..draft(
                    "Helper",
                    graph(
                        helper,
                        vec![
                            wire("called", "out", "move", "in"),
                            wire("called", "payload", "move", "amount"),
                            wire("move", "out", "back", "in"),
                            wire("answer", "value", "back", "value"),
                        ],
                    ),
                )
            },
            now_secs(),
        )
        .unwrap()
        .workflow;
    let caller = vec![
        node("t", "trigger.manual", json!({})),
        node(
            "call",
            "action.call",
            json!({ "workflow": helper.id, "payload": "200" }),
        ),
        node(
            "said",
            "var.set",
            json!({ "name": "answer", "scope": "global" }),
        ),
    ];
    let caller = save(
        &mut vault,
        "Caller",
        graph(
            caller,
            vec![
                wire("t", "out", "call", "in"),
                wire("call", "out", "said", "in"),
                wire("call", "result", "said", "value"),
            ],
        ),
    );
    let report = vault
        .run_workflow_now(&caller.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    assert_eq!(
        report.run.created.len(),
        1,
        "the helper's transfer belongs to the caller's run"
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("200"));
    assert_eq!(
        vault.wf_var(&VarScope::Global, "answer"),
        Some(WfValue::Text("moved".into()))
    );
    assert!(
        report
            .run
            .trace
            .iter()
            .any(|step| step.text_en.starts_with("↳ «Helper»"))
    );
}

#[test]
fn a_failing_helper_rolls_back_the_whole_run() {
    let (_dir, mut vault, card, savings) = wallets();
    let helper = vec![
        node("called", "trigger.called", json!({})),
        transfer_node("move", &card, &savings, "5000"),
    ];
    let helper = vault
        .save_workflow(
            None,
            WorkflowDraft {
                active: false,
                ..draft(
                    "Too much",
                    graph(helper, vec![wire("called", "out", "move", "in")]),
                )
            },
            now_secs(),
        )
        .unwrap()
        .workflow;
    let caller = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("first", &card, &savings, "100"),
        node("call", "action.call", json!({ "workflow": helper.id })),
    ];
    let caller = save(
        &mut vault,
        "Caller",
        graph(
            caller,
            vec![
                wire("t", "out", "first", "in"),
                wire("first", "out", "call", "in"),
            ],
        ),
    );
    let report = vault
        .run_workflow_now(&caller.id, None, later(), "UTC")
        .unwrap();
    assert!(
        matches!(report.run.outcome, RunOutcome::Failed { .. }),
        "{:?}",
        report.run.outcome
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
}

#[test]
fn a_workflow_calling_itself_stops_at_the_depth_limit() {
    let (_dir, mut vault) = empty_vault();
    let id = "loop-self";
    let nodes = vec![
        node("called", "trigger.called", json!({})),
        node("again", "action.call", json!({ "workflow": id })),
    ];
    let saved = vault
        .save_workflow(
            None,
            WorkflowDraft {
                active: false,
                ..draft(
                    "Loop",
                    graph(nodes, vec![wire("called", "out", "again", "in")]),
                )
            },
            now_secs(),
        )
        .unwrap()
        .workflow;
    // Point the call at the workflow itself.
    let graph_json = saved.graph.replace(id, &saved.id);
    vault.doc.workflows[0].graph = graph_json.clone();
    let report = vault
        .dry_run_workflow(
            &graph_json,
            Some(&saved.id),
            None,
            BTreeMap::new(),
            later(),
            "UTC",
        )
        .unwrap();
    assert!(
        matches!(&report.run.outcome, RunOutcome::Failed { message } if message.contains("deeper")),
        "{:?}",
        report.run.outcome
    );
}

#[test]
fn tests_compare_an_emulation_with_what_is_expected() {
    let (_dir, mut vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("tx", &card, &savings, "300"),
    ];
    let workflow = save(
        &mut vault,
        "Tested",
        graph(nodes, vec![wire("t", "out", "tx", "in")]),
    );
    let test = |name: &str, transactions: u32| WorkflowTest {
        id: String::new(),
        name: name.into(),
        trigger_node: None,
        at: None,
        answers: None,
        expect: TestExpect {
            outcome: "done".into(),
            transactions: Some(transactions),
            queued: None,
            notices: None,
        },
    };
    vault
        .save_workflow_test(&workflow.id, test("one transfer", 1))
        .unwrap();
    let saved = vault
        .save_workflow_test(&workflow.id, test("two transfers", 2))
        .unwrap();
    assert_eq!(saved.tests.len(), 2);
    let before = vault.doc.transactions.len();
    let results = vault
        .run_workflow_tests(&workflow.id, later(), "UTC")
        .unwrap();
    assert_eq!(vault.doc.transactions.len(), before, "tests write nothing");
    assert!(results[0].passed, "{:?}", results[0].problems_en);
    assert!(!results[1].passed);
    assert_eq!(
        results[1].problems_en,
        vec!["operations: 1 instead of 2".to_string()]
    );
    // Saving the graph keeps the tests.
    let kept = vault
        .save_workflow(
            Some(&workflow.id),
            draft("Tested", workflow.graph.clone()),
            now_secs(),
        )
        .unwrap();
    assert_eq!(kept.workflow.tests.len(), 2);
    let removed = vault
        .delete_workflow_test(&workflow.id, &saved.tests[1].id)
        .unwrap();
    assert_eq!(removed.tests.len(), 1);
    assert!(
        vault
            .save_workflow_test(
                &workflow.id,
                WorkflowTest {
                    expect: TestExpect {
                        outcome: "maybe".into(),
                        ..saved.tests[0].expect.clone()
                    },
                    ..saved.tests[0].clone()
                }
            )
            .is_err()
    );
}

fn count_of(vault: &UnlockedVault, workflow: &Workflow, name: &str) -> Option<Amount> {
    vault
        .wf_var(&VarScope::Workflow(workflow.id.clone()), name)
        .and_then(|value| value.as_number())
}

/// «At the start of a month: the salary day of this month, moved back to a
/// working day if it falls on a day off, and a reminder to book it».
fn payday_graph(month: u32, day: u32) -> String {
    graph(
        vec![
            node("t", "trigger.manual", json!({})),
            node(
                "make",
                "date.make",
                json!({ "year": "2026", "month": month.to_string(), "day": day }),
            ),
            node(
                "shift",
                "date.shift_workday",
                json!({ "direction": "back" }),
            ),
            node(
                "remind",
                "action.reminder",
                json!({ "text": "Внести поступление зарплаты", "time": "10:00" }),
            ),
        ],
        vec![
            wire("make", "date", "shift", "date"),
            wire("shift", "date", "remind", "date"),
            wire("t", "out", "remind", "in"),
        ],
    )
}

#[test]
fn a_payday_on_a_day_off_moves_back_to_the_working_day_before() {
    for (month, day, due) in [
        // April 25, 2026 is a Saturday: paid on Friday the 24th.
        (4, 25, "2026-04-24T10:00:00Z"),
        // June 12 is a holiday (a Friday): Thursday the 11th.
        (6, 12, "2026-06-11T10:00:00Z"),
        // A working day stays.
        (6, 10, "2026-06-10T10:00:00Z"),
        // The 31st of a 30-day month is its last day: September 30, a Wednesday.
        (9, 31, "2026-09-30T10:00:00Z"),
    ] {
        let (_dir, mut vault) = empty_vault();
        let workflow = save(&mut vault, "Payday", payday_graph(month, day));
        let report = vault
            .run_workflow_now(&workflow.id, None, later(), "UTC")
            .unwrap();
        assert_eq!(
            report.run.outcome,
            RunOutcome::Done,
            "{:?}",
            report.run.trace
        );
        assert_eq!(report.reminders.len(), 1);
        assert_eq!(report.reminders[0].due_at, due, "{month}/{day}");
        assert!(
            vault
                .doc
                .reminders
                .iter()
                .any(|reminder| reminder.due_at == due)
        );
    }
}

#[test]
fn the_payday_template_runs_as_emulation() {
    let (_dir, vault) = empty_vault();
    let report = vault
        .dry_run_workflow(
            &payday_graph(4, 25),
            None,
            None,
            BTreeMap::new(),
            later(),
            "UTC",
        )
        .unwrap();
    assert_eq!(report.reminders.len(), 1);
    // Emulation keeps nothing.
    assert!(vault.doc.reminders.is_empty());
}

#[test]
fn loops_repeat_count_and_break() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("rep", "loop.repeat", json!({ "count": "3" })),
        node("add", "var.add", json!({ "name": "repeats", "delta": "1" })),
        node("range", "loop.range", json!({ "from": "1", "to": "4" })),
        node("sum", "var.add", json!({ "name": "sum" })),
        node("many", "loop.repeat", json!({ "count": "10" })),
        node("cmp", "logic.compare", json!({ "op": "gte", "b": "2" })),
        node("branch", "logic.branch", json!({})),
        node("stop", "loop.break", json!({})),
        node("tick", "var.add", json!({ "name": "ticks", "delta": "1" })),
    ];
    let edges = vec![
        wire("t", "out", "rep", "in"),
        wire("rep", "each", "add", "in"),
        wire("rep", "done", "range", "in"),
        wire("range", "each", "sum", "in"),
        wire("range", "value", "sum", "delta"),
        wire("range", "done", "many", "in"),
        wire("many", "each", "tick", "in"),
        wire("tick", "out", "branch", "in"),
        wire("many", "index", "cmp", "a"),
        wire("cmp", "result", "branch", "condition"),
        wire("branch", "true", "stop", "in"),
    ];
    let workflow = save(&mut vault, "Loops", graph(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    assert_eq!(count_of(&vault, &workflow, "repeats"), Some(amount("3")));
    assert_eq!(count_of(&vault, &workflow, "sum"), Some(amount("10")));
    // Passes 0, 1, 2: the third one breaks out.
    assert_eq!(count_of(&vault, &workflow, "ticks"), Some(amount("3")));
}

#[test]
fn a_while_loop_sees_the_variable_it_changes() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("reset", "var.set", json!({ "name": "n", "value": "0" })),
        node("get", "var.get", json!({ "name": "n" })),
        node("cmp", "logic.compare", json!({ "op": "lt", "b": "5" })),
        node("while", "loop.while", json!({ "max": "100" })),
        node("add", "var.add", json!({ "name": "n", "delta": "1" })),
    ];
    let edges = vec![
        wire("t", "out", "reset", "in"),
        wire("reset", "out", "while", "in"),
        wire("get", "value", "cmp", "a"),
        wire("cmp", "result", "while", "condition"),
        wire("while", "each", "add", "in"),
    ];
    let workflow = save(&mut vault, "While", graph(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    assert_eq!(count_of(&vault, &workflow, "n"), Some(amount("5")));
}

#[test]
fn a_variable_read_after_a_write_is_fresh() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("one", "var.set", json!({ "name": "x", "value": "1" })),
        node("get", "var.get", json!({ "name": "x" })),
        node("say1", "action.notify", json!({})),
        node("two", "var.set", json!({ "name": "x", "value": "2" })),
        node("say2", "action.notify", json!({})),
    ];
    let edges = vec![
        wire("t", "out", "one", "in"),
        wire("one", "out", "say1", "in"),
        wire("say1", "out", "two", "in"),
        wire("two", "out", "say2", "in"),
        wire("get", "value", "say1", "text"),
        wire("get", "value", "say2", "text"),
    ];
    let workflow = save(&mut vault, "Fresh", graph(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    let bodies: Vec<&str> = report
        .notices
        .iter()
        .map(|notice| notice.body.as_str())
        .collect();
    assert_eq!(bodies, ["1", "2"]);
}

#[test]
fn a_write_apart_from_the_flow_runs_at_the_end() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node(
            "main",
            "var.set",
            json!({ "name": "seen", "value": "flow" }),
        ),
        node("apart", "var.set", json!({ "name": "apart", "value": "7" })),
    ];
    let workflow = save(
        &mut vault,
        "Apart",
        graph(nodes, vec![wire("t", "out", "main", "in")]),
    );
    assert!(
        !validate(&Graph::parse(&workflow.graph).unwrap())
            .iter()
            .any(|issue| issue.code == "unreachable"),
        "a write apart from the flow is not a mistake"
    );
    vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(count_of(&vault, &workflow, "apart"), Some(amount("7")));
}

#[test]
fn text_and_conversion_nodes_read_what_people_write() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node(
            "sms",
            "data.constant",
            json!({ "value_type": "text", "text": "Покупка 1 234,56 р. Карта *4321. 25.11.2026" }),
        ),
        node("num", "text.number", json!({})),
        node(
            "card",
            "text.extract",
            json!({ "pattern": r"\*(\d{4})", "group": "1" }),
        ),
        node(
            "when",
            "text.extract",
            json!({ "pattern": r"\d{2}\.\d{2}\.\d{4}" }),
        ),
        node("date", "convert.to", json!({ "to": "date" })),
        node("parts", "date.parts", json!({})),
        node("amount", "var.set", json!({ "name": "amount" })),
        node("digits", "var.set", json!({ "name": "card" })),
        node("month", "var.set", json!({ "name": "month" })),
    ];
    let edges = vec![
        wire("sms", "value", "num", "text"),
        wire("sms", "value", "card", "text"),
        wire("sms", "value", "when", "text"),
        wire("when", "match", "date", "value"),
        wire("date", "result", "parts", "date"),
        wire("num", "number", "amount", "value"),
        wire("card", "match", "digits", "value"),
        wire("parts", "month", "month", "value"),
        wire("t", "out", "amount", "in"),
        wire("amount", "out", "digits", "in"),
        wire("digits", "out", "month", "in"),
    ];
    let workflow = save(&mut vault, "Sms", graph(nodes, edges));
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
    assert_eq!(
        count_of(&vault, &workflow, "amount"),
        Some(amount("1234.56"))
    );
    assert_eq!(
        vault.wf_var(&scope, "card").map(|value| value.as_text()),
        Some("4321".to_string())
    );
    assert_eq!(count_of(&vault, &workflow, "month"), Some(amount("11")));
}

#[test]
fn a_draft_waits_aside_and_saving_takes_it_in() {
    let (_dir, mut vault) = empty_vault();
    let first = graph(vec![node("t", "trigger.manual", json!({}))], vec![]);
    let workflow = save(&mut vault, "Draft", first.clone());
    let edited = graph(
        vec![
            node("t", "trigger.manual", json!({})),
            node("n", "action.notify", json!({ "text": "hi" })),
        ],
        vec![wire("t", "out", "n", "in")],
    );
    let drafted = vault
        .save_workflow_draft(&workflow.id, &edited, later())
        .unwrap();
    // The saved graph is untouched; the draft sits next to it.
    assert_eq!(drafted.graph, workflow.graph);
    assert!(
        drafted
            .draft
            .as_ref()
            .is_some_and(|draft| draft.graph.contains("action.notify"))
    );
    assert!(drafted.versions.is_empty());

    let dropped = vault.discard_workflow_draft(&workflow.id).unwrap();
    assert!(dropped.draft.is_none());

    vault
        .save_workflow_draft(&workflow.id, &edited, later())
        .unwrap();
    let saved = vault
        .save_workflow(Some(&workflow.id), draft("Draft", edited), later())
        .unwrap()
        .workflow;
    assert!(saved.draft.is_none());
    assert!(vault.save_workflow_draft("nope", &first, later()).is_err());
}

#[test]
fn an_emulation_shows_the_workflows_its_events_start() {
    let (_dir, mut vault) = empty_vault();
    let listener = vec![
        node("on", "trigger.event", json!({ "name": "paid" })),
        node("n", "action.notify", json!({})),
    ];
    let listener = save(
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
    let sender = graph(
        vec![
            node("t", "trigger.manual", json!({})),
            node(
                "e",
                "action.emit",
                json!({ "name": "paid", "payload": "ипотека" }),
            ),
        ],
        vec![wire("t", "out", "e", "in")],
    );
    let report = vault
        .dry_run_workflow(&sender, None, None, BTreeMap::new(), later(), "UTC")
        .unwrap();
    assert_eq!(report.event_runs.len(), 1);
    let run = &report.event_runs[0];
    assert_eq!(run.workflow_id, listener.id);
    assert!(run.dry_run);
    assert_eq!(run.outcome, RunOutcome::Done);
    // Nothing of it is kept: not the run, not the notice.
    assert!(vault.doc.workflow_runs.is_empty());
    assert!(vault.notifications().is_empty());
}

#[test]
fn an_emulated_wait_moves_the_clock_for_what_follows() {
    let (_dir, vault) = empty_vault();
    let graph_json = graph(
        vec![
            node("t", "trigger.manual", json!({})),
            node("w", "action.wait", json!({ "hours": "48" })),
            node("now", "data.now", json!({})),
            node("set", "var.set", json!({ "name": "when" })),
            node(
                "remind",
                "action.reminder",
                json!({ "text": "later", "time": "10:00" }),
            ),
        ],
        vec![
            wire("t", "out", "w", "in"),
            wire("w", "out", "set", "in"),
            wire("set", "out", "remind", "in"),
            wire("now", "now", "set", "value"),
        ],
    );
    let start = later();
    let report = vault
        .dry_run_workflow(&graph_json, None, None, BTreeMap::new(), start, "UTC")
        .unwrap();
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    // «Now» after the wait is two days on, and so is today's reminder.
    let two_days_on = crate::calendar::iso(start + 48 * 3600);
    let seen = report
        .edge_values
        .get("now.now>set.value")
        .cloned()
        .unwrap_or_default();
    assert_eq!(seen, two_days_on);
    assert_eq!(report.reminders[0].due_at.get(..10), two_days_on.get(..10));
}

#[test]
fn a_question_without_text_is_left_for_the_app_to_word() {
    let (_dir, mut vault) = empty_vault();
    let workflow = save(
        &mut vault,
        "Ask",
        graph(
            vec![
                node("t", "trigger.manual", json!({})),
                node("c", "action.confirm", json!({})),
            ],
            vec![wire("t", "out", "c", "in")],
        ),
    );
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert!(
        matches!(&report.run.outcome, RunOutcome::Asking { question, .. } if question.is_empty()),
        "{:?}",
        report.run.outcome
    );
    let step = report.run.trace.last().unwrap();
    assert!(step.text_ru.contains("Продолжить?"), "{}", step.text_ru);
    assert!(step.text_en.contains("Go on?"), "{}", step.text_en);
    assert!(vault.workflow_waits()[0].question.is_empty());
}

/* --- data order, branches, waits (graph v3) ----------------------------------- */

fn graph_v3(nodes: Vec<Value>, edges: Vec<Value>) -> String {
    json!({ "version": 3, "nodes": nodes, "edges": edges }).to_string()
}

/// The trace's node ids in the order they ran.
fn ran_order(report: &RunReport) -> Vec<String> {
    report
        .run
        .trace
        .iter()
        .map(|step| step.node_id.clone())
        .collect()
}

#[test]
fn an_action_runs_after_the_action_it_takes_data_from_without_a_flow_wire() {
    let (_dir, mut vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("pay", &card, &savings, "100"),
        node("n", "action.notify", json!({})),
    ];
    let edges = vec![
        wire("t", "out", "pay", "in"),
        wire("pay", "transaction", "n", "text"),
    ];
    let workflow = save(&mut vault, "Order", graph_v3(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(report.run.outcome, RunOutcome::Done);
    assert_eq!(report.notices.len(), 1);
    let order = ran_order(&report);
    let at = |id: &str| order.iter().position(|step| step == id).unwrap();
    assert!(at("pay") < at("n"), "{order:?}");
}

#[test]
fn an_action_waiting_on_a_branch_that_did_not_run_does_not_run() {
    let (_dir, mut vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        // 0 days > 0 is false: the branch goes «No».
        node("diff", "date.diff", json!({})),
        node("cmp", "logic.compare", json!({ "op": "gt", "b": "0" })),
        node("b", "logic.branch", json!({})),
        transfer_node("pay", &card, &savings, "100"),
        node("n", "action.notify", json!({})),
    ];
    let edges = vec![
        wire("diff", "days", "cmp", "a"),
        wire("cmp", "result", "b", "condition"),
        wire("t", "out", "b", "in"),
        wire("b", "true", "pay", "in"),
        wire("pay", "transaction", "n", "text"),
    ];
    let workflow = save(&mut vault, "Gated", graph_v3(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert!(report.transactions.is_empty());
    assert!(
        report.notices.is_empty(),
        "the notice waits on a payment that never happened"
    );
}

#[test]
fn data_order_reaches_through_pure_data_nodes() {
    let (_dir, mut vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("pay", &card, &savings, "100"),
        node("text", "logic.text", json!({ "template": "Ушло: {a}" })),
        node("n", "action.notify", json!({})),
    ];
    let edges = vec![
        wire("t", "out", "pay", "in"),
        wire("pay", "transaction", "text", "a"),
        wire("text", "text", "n", "text"),
    ];
    let workflow = save(&mut vault, "Through", graph_v3(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert_eq!(report.notices.len(), 1);
    assert!(
        report.notices[0].body.starts_with("Ушло:"),
        "{}",
        report.notices[0].body
    );
}

#[test]
fn a_failing_branch_is_undone_and_its_sibling_goes_on() {
    let (_dir, mut vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        // More than the card holds: fails, with no «Error» way.
        transfer_node("bad", &card, &savings, "99999"),
        transfer_node("good", &card, &savings, "100"),
    ];
    let edges = vec![
        wire("t", "out", "bad", "in"),
        wire("t", "out", "good", "in"),
    ];
    let workflow = save(&mut vault, "Branches", graph_v3(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert!(
        matches!(&report.run.outcome, RunOutcome::Partial { failed } if failed.len() == 1),
        "{:?}",
        report.run.outcome
    );
    assert_eq!(report.transactions.len(), 1, "the good branch is kept");
}

#[test]
fn a_failure_on_the_main_line_still_undoes_the_whole_run() {
    let (_dir, mut vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("good", &card, &savings, "100"),
        transfer_node("bad", &card, &savings, "99999"),
    ];
    let edges = vec![
        wire("t", "out", "good", "in"),
        wire("good", "out", "bad", "in"),
    ];
    let workflow = save(&mut vault, "Line", graph_v3(nodes, edges));
    let before = vault.doc.transactions.len();
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert!(
        matches!(report.run.outcome, RunOutcome::Failed { .. }),
        "{:?}",
        report.run.outcome
    );
    assert_eq!(vault.doc.transactions.len(), before);
}

#[test]
fn a_nested_fork_catches_its_own_failure() {
    let (_dir, mut vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("a", &card, &savings, "10"),
        transfer_node("b", &card, &savings, "20"),
        transfer_node("bad", &card, &savings, "99999"),
        transfer_node("c", &card, &savings, "30"),
    ];
    let edges = vec![
        wire("t", "out", "a", "in"),
        wire("t", "out", "c", "in"),
        wire("a", "out", "bad", "in"),
        wire("a", "out", "b", "in"),
    ];
    let workflow = save(&mut vault, "Nested", graph_v3(nodes, edges));
    let report = vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    assert!(
        matches!(report.run.outcome, RunOutcome::Partial { .. }),
        "{:?}",
        report.run.outcome
    );
    // «a» and «b» around the failed «bad», and «c» on the other side.
    assert_eq!(report.transactions.len(), 3);
}

#[test]
fn an_older_graph_keeps_switched_off_what_data_order_would_wake() {
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("pay", "action.transaction", json!({})),
        node("late", "action.transaction", json!({})),
        node("n", "action.notify", json!({})),
    ];
    let edges = vec![
        wire("t", "out", "pay", "in"),
        wire("pay", "transaction", "late", "amount"),
        wire("pay", "transaction", "n", "text"),
    ];
    let json = json!({ "version": 2, "nodes": nodes, "edges": edges }).to_string();
    let graph = Graph::parse(&json).unwrap();
    assert_eq!(graph.version, crate::workflow::graph::GRAPH_VERSION);
    let disabled = |id: &str| graph.node(id).unwrap().disabled;
    assert!(disabled("late"), "it never ran before: it stays that way");
    assert!(!disabled("n"), "a notice apart from the flow always ran");
    assert!(!disabled("pay"));
}

#[test]
fn a_flow_wire_that_repeats_a_data_wire_is_pointed_out() {
    let (_dir, _vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        transfer_node("pay", &card, &savings, "100"),
        node("n", "action.notify", json!({})),
    ];
    let edges = vec![
        wire("t", "out", "pay", "in"),
        wire("pay", "out", "n", "in"),
        wire("pay", "transaction", "n", "text"),
    ];
    let graph = Graph::parse(&graph_v3(nodes, edges)).unwrap();
    let issues = validate(&graph);
    assert!(
        issues.iter().any(|issue| issue.code == "redundant_flow"
            && issue.edge_id.as_deref() == Some("pay.out>n.in")),
        "{issues:?}"
    );
}

#[test]
fn reading_an_action_that_runs_later_is_a_warning() {
    let (_dir, _vault, card, savings) = wallets();
    let nodes = vec![
        node("t", "trigger.manual", json!({})),
        node("n", "action.notify", json!({})),
        transfer_node("pay", &card, &savings, "100"),
    ];
    let edges = vec![
        wire("t", "out", "n", "in"),
        wire("t", "out", "pay", "in"),
        wire("pay", "transaction", "n", "text"),
    ];
    let graph = Graph::parse(&graph_v3(nodes, edges)).unwrap();
    let issues = validate(&graph);
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "late_data" && issue.node_id.as_deref() == Some("n")),
        "{issues:?}"
    );
}

/// «Run → wait 2 h → notice», with a free counter that adds up on each finish.
fn waiting_graph() -> String {
    graph_v3(
        vec![
            node("t", "trigger.manual", json!({})),
            node("w", "action.wait", json!({ "hours": "2" })),
            node("n", "action.notify", json!({ "text": "готово" })),
            node(
                "count",
                "var.add",
                json!({ "name": "finished", "value": "1" }),
            ),
        ],
        vec![wire("t", "out", "w", "in"), wire("w", "out", "n", "in")],
    )
}

#[test]
fn free_nodes_run_after_the_last_wait() {
    let (_dir, mut vault) = empty_vault();
    let workflow = save(&mut vault, "Free", waiting_graph());
    let now = later();
    vault
        .run_workflow_now(&workflow.id, None, now, "UTC")
        .unwrap();
    assert_eq!(
        count_of(&vault, &workflow, "finished"),
        None,
        "not before the wait is over"
    );
    vault.run_due_workflows(now + 2 * 3600, "UTC").unwrap();
    assert_eq!(
        count_of(&vault, &workflow, "finished"),
        Some(Amount::new(rust_decimal::Decimal::ONE))
    );
}

#[test]
fn a_sleeping_run_goes_on_along_the_graph_it_fell_asleep_on() {
    let (_dir, mut vault) = empty_vault();
    let workflow = save(&mut vault, "Edited", waiting_graph());
    let now = later();
    vault
        .run_workflow_now(&workflow.id, None, now, "UTC")
        .unwrap();
    // Edited while asleep: the notice is gone from the current graph.
    let edited = graph_v3(
        vec![
            node("t", "trigger.manual", json!({})),
            node("w", "action.wait", json!({ "hours": "2" })),
        ],
        vec![wire("t", "out", "w", "in")],
    );
    vault
        .save_workflow(Some(&workflow.id), draft("Edited", edited), now)
        .unwrap();
    let pass = vault.run_due_workflows(now + 2 * 3600, "UTC").unwrap();
    assert_eq!(
        pass.notices.len(),
        1,
        "the run had a notice when it fell asleep"
    );
}

#[test]
fn a_wait_of_a_switched_off_workflow_is_journalled_as_dropped() {
    let (_dir, mut vault) = empty_vault();
    let workflow = save(&mut vault, "Off", waiting_graph());
    let now = later();
    vault
        .run_workflow_now(&workflow.id, None, now, "UTC")
        .unwrap();
    vault.set_workflow_active(&workflow.id, false, now).unwrap();
    let pass = vault.run_due_workflows(now + 2 * 3600, "UTC").unwrap();
    assert!(pass.notices.is_empty());
    assert!(
        pass.runs.iter().any(|run| run.outcome
            == RunOutcome::Dropped {
                reason: "off".into()
            }),
        "{:?}",
        pass.runs
    );
    assert!(vault.doc.workflow_waits.is_empty());
}

#[test]
fn a_question_of_a_switched_off_workflow_cannot_be_answered() {
    let (_dir, mut vault, card, savings) = wallets();
    let workflow = save(&mut vault, "Ask", ask_graph(&card, &savings, ""));
    let now = later();
    vault
        .run_workflow_now(&workflow.id, None, now, "UTC")
        .unwrap();
    let wait = vault.doc.workflow_waits[0].id.clone();
    vault.set_workflow_active(&workflow.id, false, now).unwrap();
    assert!(vault.answer_workflow_wait(&wait, true, now, "UTC").is_err());
    assert_eq!(
        vault.doc.workflow_waits.len(),
        1,
        "the question stays for later"
    );
}

#[test]
fn a_wait_from_an_older_app_still_reads() {
    let old = json!({
        "id": "w1", "workflow_id": "wf", "run_id": "r", "node_id": "n",
        "resume_at": "", "memo": "[]", "queue_mode": true, "trigger": "manual",
    });
    let from_json: WorkflowWait = serde_json::from_value(old.clone()).unwrap();
    assert!(from_json.graph.is_empty());
    let packed = rmp_serde::to_vec_named(&old).unwrap();
    let from_pack: WorkflowWait = rmp_serde::from_slice(&packed).unwrap();
    assert_eq!(from_pack, from_json);
}

#[test]
fn run_now_from_a_schedule_takes_its_latest_moment_and_says_by_hand() {
    let (_dir, mut vault) = empty_vault();
    let nodes = vec![
        node("m", "trigger.manual", json!({})),
        node(
            "s",
            "trigger.schedule",
            json!({ "every": "day", "time": "09:00" }),
        ),
        node("text", "logic.text", json!({ "template": "{a}" })),
        node("n", "action.notify", json!({})),
    ];
    let edges = vec![
        wire("m", "out", "n", "in"),
        wire("s", "out", "n", "in"),
        wire("s", "at", "text", "a"),
        wire("text", "text", "n", "text"),
    ];
    let workflow = save(&mut vault, "Pick", graph_v3(nodes, edges));
    let now = unix_secs("2026-03-05T15:30:00Z").unwrap();
    let report = vault
        .run_workflow_now(&workflow.id, Some("s"), now, "UTC")
        .unwrap();
    assert_eq!(report.run.trigger, "schedule by hand");
    assert!(
        report.notices[0].body.contains("2026-03-05T09:00:00"),
        "{}",
        report.notices[0].body
    );
    // With no trigger named, the button starts «Запуск вручную».
    let manual = vault
        .run_workflow_now(&workflow.id, None, now, "UTC")
        .unwrap();
    assert_eq!(manual.run.trigger, "manual");
}

/// Salary → «Очередь» → send the salary's amount from savings to the card.
/// Savings hold 500; the card gets the salaries.
fn queue_vault(mode: Option<&str>) -> (tempfile::TempDir, UnlockedVault, String, Workflow) {
    let (dir, mut vault, card, savings) = wallets();
    vault
        .add_transaction(income(&savings, "500", "opening"))
        .unwrap();
    let pay = transfer_node("pay", &savings, &card, "0");
    let trigger = node(
        "t",
        "trigger.transaction",
        json!({ "tx_kind": "topup", "category": "salary" }),
    );
    let (nodes, edges) = match mode {
        Some(mode) => (
            vec![
                trigger,
                node("q", "flow.queue", json!({ "mode": mode })),
                pay,
            ],
            vec![
                wire("t", "out", "q", "in"),
                wire("q", "each", "pay", "in"),
                wire("t", "amount", "pay", "amount"),
            ],
        ),
        None => (
            vec![trigger, pay],
            vec![
                wire("t", "out", "pay", "in"),
                wire("t", "amount", "pay", "amount"),
            ],
        ),
    };
    let workflow = save(&mut vault, "Queue", graph_v3(nodes, edges));
    (dir, vault, savings, workflow)
}

fn salaries(vault: &mut UnlockedVault, card: &str, amounts: &[&str]) {
    for value in amounts {
        vault
            .add_transaction(income(card, value, "salary"))
            .unwrap();
    }
}

fn card_of(vault: &UnlockedVault, savings: &str) -> String {
    vault
        .doc
        .stores
        .iter()
        .find(|store| store.id != savings && store.name == "Card")
        .map(|store| store.id.clone())
        .unwrap()
}

#[test]
fn a_queue_takes_the_firings_of_a_pass_in_one_run() {
    let (_dir, mut vault, savings, workflow) = queue_vault(Some("sequence"));
    let card = card_of(&vault, &savings);
    salaries(&mut vault, &card, &["100", "50", "200"]);
    let pass = vault.run_due_workflows(later(), "UTC").unwrap();
    assert_eq!(pass.runs.len(), 1, "{:?}", pass.runs);
    assert_eq!(
        pass.runs[0].outcome,
        RunOutcome::Done,
        "{:?}",
        pass.runs[0].trace
    );
    assert!(
        pass.runs[0].trigger.contains("×3"),
        "{}",
        pass.runs[0].trigger
    );
    assert_eq!(pass.created.len(), 3);
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("150"));
    assert_eq!(vault.workflow_runs(Some(&workflow.id), 10).len(), 1);
}

#[test]
fn a_queue_in_turn_undoes_everything_on_a_failure() {
    let (_dir, mut vault, savings, _workflow) = queue_vault(Some("sequence"));
    let card = card_of(&vault, &savings);
    salaries(&mut vault, &card, &["100", "1000", "200"]);
    let pass = vault.run_due_workflows(later(), "UTC").unwrap();
    assert_eq!(pass.runs.len(), 1);
    assert!(
        matches!(pass.runs[0].outcome, RunOutcome::Failed { .. }),
        "{:?}",
        pass.runs[0].outcome
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("500"));
}

#[test]
fn an_independent_queue_keeps_the_firings_that_went_through() {
    let (_dir, mut vault, savings, _workflow) = queue_vault(Some("independent"));
    let card = card_of(&vault, &savings);
    salaries(&mut vault, &card, &["100", "1000", "200"]);
    let pass = vault.run_due_workflows(later(), "UTC").unwrap();
    assert_eq!(pass.runs.len(), 1);
    assert!(
        matches!(&pass.runs[0].outcome, RunOutcome::Partial { failed } if failed.len() == 1),
        "{:?}",
        pass.runs[0].outcome
    );
    assert_eq!(pass.created.len(), 2);
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("200"));
}

#[test]
fn without_a_queue_every_firing_is_its_own_run() {
    let (_dir, mut vault, savings, _workflow) = queue_vault(None);
    let card = card_of(&vault, &savings);
    salaries(&mut vault, &card, &["100", "50", "200"]);
    let pass = vault.run_due_workflows(later(), "UTC").unwrap();
    assert_eq!(pass.runs.len(), 3, "{:?}", pass.runs);
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("150"));
}

#[test]
fn a_queue_stands_after_a_trigger_and_does_not_sleep() {
    let lone = crate::workflow::Graph::parse(&graph_v3(
        vec![
            node("t", "trigger.manual", json!({})),
            node("q", "flow.queue", json!({})),
        ],
        vec![],
    ))
    .unwrap();
    let codes: Vec<_> = crate::workflow::validate(&lone)
        .into_iter()
        .map(|issue| issue.code)
        .collect();
    assert!(
        codes.contains(&"queue_without_trigger".to_string()),
        "{codes:?}"
    );

    let sleepy = crate::workflow::Graph::parse(&graph_v3(
        vec![
            node("t", "trigger.manual", json!({})),
            node("q", "flow.queue", json!({})),
            node("w", "action.wait", json!({ "hours": "1" })),
        ],
        vec![wire("t", "out", "q", "in"), wire("q", "each", "w", "in")],
    ))
    .unwrap();
    let codes: Vec<_> = crate::workflow::validate(&sleepy)
        .into_iter()
        .map(|issue| issue.code)
        .collect();
    assert!(codes.contains(&"queue_sleeps".to_string()), "{codes:?}");
    assert!(
        !codes.contains(&"queue_without_trigger".to_string()),
        "{codes:?}"
    );
}

/// A price watch with no asset: it could never fire.
fn price_watch(asset: Value) -> String {
    graph(
        vec![
            node(
                "t",
                "trigger.price",
                json!({ "asset": asset, "condition": "above", "value": "1" }),
            ),
            node("n", "action.notify", json!({ "text": "up" })),
        ],
        vec![wire("t", "out", "n", "in")],
    )
}

#[test]
fn a_draft_with_an_empty_required_field_saves_inactive() {
    let (_dir, mut vault) = empty_vault();
    let saved = vault
        .save_workflow(
            None,
            WorkflowDraft {
                active: false,
                ..draft("Watch", price_watch(Value::Null))
            },
            now_secs(),
        )
        .unwrap();
    assert!(!saved.workflow.active);
    assert!(
        saved
            .issues
            .iter()
            .any(|issue| issue.code == "param_missing")
    );
}

#[test]
fn activating_with_an_empty_required_field_is_refused() {
    let (_dir, mut vault) = empty_vault();
    assert!(
        vault
            .save_workflow(None, draft("Watch", price_watch(Value::Null)), now_secs())
            .is_err()
    );
    let off = vault
        .save_workflow(
            None,
            WorkflowDraft {
                active: false,
                ..draft("Watch", price_watch(Value::Null))
            },
            now_secs(),
        )
        .unwrap()
        .workflow;
    assert!(
        vault
            .set_workflow_active(&off.id, true, now_secs())
            .is_err()
    );
}

#[test]
fn opening_a_vault_turns_off_workflows_missing_a_required_field() {
    let (dir, mut vault) = empty_vault();
    let on = save(&mut vault, "Watch", price_watch(json!("BTC")));
    // As if the field had become required after it was switched on.
    vault.doc.workflows[0].graph = price_watch(Value::Null);
    vault.persist().unwrap();
    drop(vault);

    let reopened = UnlockedVault::open(dir.path().join("test.wallet"), "pw").unwrap();
    let workflow = reopened.workflow(&on.id).unwrap();
    assert!(!workflow.active);
    assert!(reopened.doc.workflow_runs.iter().any(|run| {
        run.workflow_id == on.id
            && matches!(&run.outcome, RunOutcome::Dropped { reason } if reason == "missing_params")
    }));
    assert!(
        reopened
            .doc
            .notification_log
            .iter()
            .any(|record| record.key == format!("wf-off:{}", on.id))
    );
}

fn secs_at(iso: &str) -> i64 {
    crate::pricing::history::unix_secs(iso).unwrap()
}

/// «The 20th, moved back off a day off → top the card up by the average of
/// the latest salaries, marked approximate».
fn salary_graph(card: &str) -> String {
    graph(
        vec![
            node(
                "t",
                "trigger.schedule",
                json!({ "every": "month", "day": 20, "time": "09:00", "workday": "back" }),
            ),
            node(
                "avg",
                "data.recent_average",
                json!({ "count": "3", "fallback": "1000" }),
            ),
            node(
                "tx",
                "action.transaction",
                json!({ "tx_kind": "topup", "account": store_ref(card), "asset": "USD", "category": "salary", "estimated": true }),
            ),
        ],
        vec![
            wire("t", "out", "tx", "in"),
            wire("t", "at", "tx", "date"),
            wire("avg", "amount", "tx", "amount"),
        ],
    )
}

#[test]
fn a_salary_lands_on_the_friday_before_with_the_average_amount() {
    let (_dir, mut vault, card, _) = wallets();
    let workflow = save(&mut vault, "Advance", salary_graph(&card));
    vault.wf_set_mark(&workflow.id, "schedule:t", "2026-11-21T00:00:00Z");

    // 2026-12-20 is a Sunday: the advance comes on Friday the 18th.
    let pass = vault
        .run_due_workflows(secs_at("2026-12-18T10:00:00Z"), "UTC")
        .unwrap();
    assert_eq!(pass.created.len(), 1, "{:?}", pass.runs);
    let made = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap()
        .clone();
    assert_eq!(made.meta().timestamp, "2026-12-18T09:00:00Z");
    assert!(made.meta().origin.is_estimated());
    assert!(matches!(
        &made.meta().origin,
        TxOrigin::Workflow { planned_at: Some(at), .. } if at == "2026-12-18T09:00:00Z"
    ));
    // Nothing yet to average: the fallback.
    assert_eq!(tx_amount(&made), amount("1000"));
    // Not again on the planned Sunday.
    let pass = vault
        .run_due_workflows(secs_at("2026-12-20T10:00:00Z"), "UTC")
        .unwrap();
    assert!(pass.created.is_empty());

    // The user puts in the real amount.
    correct(&mut vault, &made.meta().id, "1200");
    let pass = vault
        .run_due_workflows(secs_at("2027-01-20T10:00:00Z"), "UTC")
        .unwrap();
    assert_eq!(pass.created.len(), 1);
    let next = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap()
        .clone();
    assert_eq!(tx_amount(&next), amount("1200"));
    // Approximate ones stay out of the average.
    let pass = vault
        .run_due_workflows(secs_at("2027-02-20T10:00:00Z"), "UTC")
        .unwrap();
    let third = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap()
        .clone();
    assert_eq!(tx_amount(&third), amount("1200"));
    correct(&mut vault, &next.meta().id, "1400");
    correct(&mut vault, &third.meta().id, "1600");
    assert_eq!(
        vault.wf_recent_average(&workflow.id, 3),
        Some((amount("1400"), "USD".to_string()))
    );
}

fn tx_amount(tx: &Transaction) -> Amount {
    match tx {
        Transaction::Topup(inner) => inner.amount,
        Transaction::Payment(inner) => inner.amount,
        Transaction::Transfer(inner) => inner.amount,
        _ => Amount::ZERO,
    }
}

fn correct(vault: &mut UnlockedVault, id: &str, value: &str) {
    let tx = vault
        .doc
        .transactions
        .iter_mut()
        .find(|tx| tx.meta().id == id)
        .unwrap();
    if let Transaction::Topup(inner) = tx {
        inner.amount = amount(value);
        if let TxOrigin::Workflow { estimated, .. } = &mut inner.meta.origin {
            *estimated = false;
        }
    }
}

fn rent_graph(card: &str, savings: &str) -> String {
    graph(
        vec![
            node(
                "t",
                "trigger.schedule",
                json!({ "every": "month", "day": 1, "time": "09:00" }),
            ),
            node(
                "tx",
                "action.transaction",
                json!({ "tx_kind": "transfer", "account": store_ref(card), "to": store_ref(savings), "asset": "USD", "amount": "3000", "on_shortfall": "reject" }),
            ),
        ],
        vec![wire("t", "out", "tx", "in"), wire("t", "at", "tx", "date")],
    )
}

#[test]
fn a_refused_run_waits_for_retry_or_skip() {
    let (_dir, mut vault, card, savings) = wallets();
    let workflow = save(&mut vault, "Rent", rent_graph(&card, &savings));
    vault.wf_set_mark(&workflow.id, "schedule:t", "2026-11-20T00:00:00Z");

    let pass = vault
        .run_due_workflows(secs_at("2026-12-01T10:00:00Z"), "UTC")
        .unwrap();
    assert_eq!(pass.runs.len(), 1);
    let refused = pass.runs[0].clone();
    let RunOutcome::Rejected { planned_at, .. } = &refused.outcome else {
        panic!("{:?}", refused.outcome);
    };
    assert_eq!(planned_at.as_deref(), Some("2026-12-01T09:00:00Z"));
    assert!(pass.created.is_empty());

    // Still short: the error says so and the refusal stays.
    let err = vault
        .retry_workflow_run(&refused.id, secs_at("2026-12-02T10:00:00Z"), "UTC")
        .unwrap_err();
    assert_eq!(err.code(), "INSUFFICIENT_FUNDS");
    assert!(
        vault
            .doc
            .workflow_runs
            .iter()
            .any(|run| run.id == refused.id)
    );

    vault
        .add_transaction(income(&card, "5000", "salary"))
        .unwrap();
    let pass = vault
        .retry_workflow_run(&refused.id, secs_at("2026-12-03T10:00:00Z"), "UTC")
        .unwrap();
    assert_eq!(pass.created.len(), 1);
    assert!(
        vault
            .doc
            .workflow_runs
            .iter()
            .all(|run| run.id != refused.id)
    );
    let made = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap();
    // Dated when it was made, still naming its planned moment.
    assert_eq!(made.meta().timestamp, "2026-12-03T10:00:00Z");
    assert!(matches!(
        &made.meta().origin,
        TxOrigin::Workflow { planned_at: Some(at), .. } if at == "2026-12-01T09:00:00Z"
    ));

    // January is covered by the rest of the top-up; February is refused
    // and skipped.
    let pass = vault
        .run_due_workflows(secs_at("2027-01-01T10:00:00Z"), "UTC")
        .unwrap();
    assert_eq!(pass.created.len(), 1);
    let pass = vault
        .run_due_workflows(secs_at("2027-02-01T10:00:00Z"), "UTC")
        .unwrap();
    assert!(matches!(pass.runs[0].outcome, RunOutcome::Rejected { .. }));
    let skipped = vault.skip_workflow_run(&pass.runs[0].id).unwrap();
    assert_eq!(
        skipped.outcome,
        RunOutcome::Dropped {
            reason: "skipped".into()
        }
    );
    assert!(vault.skip_workflow_run(&pass.runs[0].id).is_err());
}
