//! One job met by two devices runs once: schedule moments, operation
//! triggers and waking waits carry a run key, and a second run of a key is
//! skipped or merged away after a sync.

use serde_json::{Value, json};

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::calendar::now_secs;
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::pricing::history::unix_secs;
use crate::service::test_support::{income, store};
use crate::service::workflows::WorkflowDraft;

fn node(id: &str, kind: &str, params: Value) -> Value {
    json!({ "id": id, "kind": kind, "params": params })
}

fn wire(from: &str, from_port: &str, to: &str, to_port: &str) -> Value {
    json!({
        "id": format!("{from}.{from_port}>{to}.{to_port}"),
        "from": from, "from_port": from_port, "to": to, "to_port": to_port,
    })
}

fn save(vault: &mut UnlockedVault, name: &str, nodes: Vec<Value>, edges: Vec<Value>) -> String {
    let draft = WorkflowDraft {
        name: name.into(),
        description: String::new(),
        active: true,
        graph: json!({ "version": 1, "nodes": nodes, "edges": edges }).to_string(),
        react_to_workflows: false,
        max_amount: None,
        max_amount_currency: None,
    };
    vault
        .save_workflow(None, draft, now_secs())
        .unwrap()
        .workflow
        .id
}

fn at(raw: &str) -> i64 {
    unix_secs(raw).unwrap()
}

fn topup_node() -> Value {
    node(
        "post",
        "action.transaction",
        json!({ "tx_kind": "topup", "asset": "USD", "amount": "10", "category": "salary", "account": { "store": "CASH" } }),
    )
}

/// «Every day at 09:00 → top up the cash wallet by 10», on two devices.
fn daily_topup() -> (tempfile::TempDir, UnlockedVault, UnlockedVault, String) {
    let (dir, mut left, mut right) = two_devices();
    let cash = left.doc.stores[0].id.clone();
    let mut post = topup_node();
    post["params"]["account"] = json!({ "store": cash });
    let id = save(
        &mut left,
        "Daily",
        vec![
            node(
                "s",
                "trigger.schedule",
                json!({ "every": "day", "time": "09:00" }),
            ),
            post,
        ],
        vec![wire("s", "out", "post", "in")],
    );
    left.wf_set_mark(&id, "schedule:s", "2026-03-01T10:00:00Z");
    left.persist().unwrap();
    settle(&mut left, &mut right);
    (dir, left, right, id)
}

fn workflow_keys(vault: &UnlockedVault) -> Vec<Option<String>> {
    let mut keys: Vec<Option<String>> = vault
        .doc
        .transactions
        .iter()
        .filter_map(|tx| match &tx.meta().origin {
            TxOrigin::Workflow { run_key, .. } => Some(run_key.clone()),
            _ => None,
        })
        .collect();
    keys.sort();
    keys
}

#[test]
fn a_schedule_moment_runs_once_across_two_devices() {
    let (_dir, mut left, mut right, _id) = daily_topup();
    let moment = at("2026-03-02T10:00:00Z");
    left.run_due_workflows(moment, "UTC").unwrap();
    right.run_due_workflows(moment, "UTC").unwrap();
    assert_eq!(workflow_keys(&left).len(), 1);
    assert_eq!(workflow_keys(&right).len(), 1);
    settle(&mut left, &mut right);
    assert_eq!(workflow_keys(&left).len(), 1, "the moment was booked twice");
    assert_eq!(workflow_keys(&right), workflow_keys(&left));
    assert!(workflow_keys(&left)[0].is_some());
}

#[test]
fn a_booked_run_key_does_not_run_again() {
    let (_dir, mut left, _right, id) = daily_topup();
    let moment = at("2026-03-02T10:00:00Z");
    left.run_due_workflows(moment, "UTC").unwrap();
    assert_eq!(workflow_keys(&left).len(), 1);
    // The mark went back (a sync of an older state): the key is still booked.
    left.wf_set_mark(&id, "schedule:s", "2026-03-01T10:00:00Z");
    let pass = left.run_due_workflows(moment, "UTC").unwrap();
    assert!(pass.created.is_empty(), "{:?}", pass.created);
    assert_eq!(workflow_keys(&left).len(), 1);
}

#[test]
fn devices_in_different_zones_compute_the_same_run_key() {
    let (_dir, mut left, mut right, _id) = daily_topup();
    left.doc.settings.schedule_timezone = Some("UTC".into());
    left.persist().unwrap();
    settle(&mut left, &mut right);
    let moment = at("2026-03-03T00:00:00Z");
    left.run_due_workflows(moment, "UTC").unwrap();
    right.run_due_workflows(moment, "Asia/Bangkok").unwrap();
    settle(&mut left, &mut right);
    let keys = workflow_keys(&left);
    assert_eq!(keys.len(), 1, "{keys:?}");
    let key = keys[0].as_deref().expect("a run key");
    assert!(key.ends_with("T09:00:00Z"), "{key}");
    assert_eq!(workflow_keys(&right), keys);
}

#[test]
fn the_first_device_that_runs_a_schedule_writes_its_zone() {
    let (_dir, mut left, mut right, _id) = daily_topup();
    assert_eq!(left.doc.settings.schedule_timezone, None);
    left.run_due_workflows(at("2026-03-02T10:00:00Z"), "Asia/Bangkok")
        .unwrap();
    assert_eq!(
        left.doc.settings.schedule_timezone.as_deref(),
        Some("Asia/Bangkok")
    );
    // The zone is kept by the next device: it does not overwrite it.
    settle(&mut left, &mut right);
    right
        .run_due_workflows(at("2026-03-03T10:00:00Z"), "UTC")
        .unwrap();
    assert_eq!(
        right.doc.settings.schedule_timezone.as_deref(),
        Some("Asia/Bangkok")
    );
}

/// «Wait 2 hours → top up», started by hand on the left device.
#[test]
fn a_wait_resumed_on_two_devices_posts_once() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = left.doc.stores[0].id.clone();
    let mut post = topup_node();
    post["params"]["account"] = json!({ "store": cash });
    let id = save(
        &mut left,
        "Later",
        vec![
            node("t", "trigger.manual", json!({})),
            node("w", "action.wait", json!({ "hours": "2" })),
            post,
        ],
        vec![wire("t", "out", "w", "in"), wire("w", "out", "post", "in")],
    );
    let start = now_secs();
    left.run_workflow_now(&id, None, start, "UTC").unwrap();
    settle(&mut left, &mut right);
    assert_eq!(right.doc.workflow_waits.len(), 1);
    left.run_due_workflows(start + 2 * 3600, "UTC").unwrap();
    right.run_due_workflows(start + 2 * 3600, "UTC").unwrap();
    assert_eq!(workflow_keys(&left).len(), 1);
    assert_eq!(workflow_keys(&right).len(), 1);
    settle(&mut left, &mut right);
    assert_eq!(workflow_keys(&left).len(), 1, "the wait posted twice");
    assert_eq!(workflow_keys(&right), workflow_keys(&left));
    assert!(left.doc.workflow_waits.is_empty() && right.doc.workflow_waits.is_empty());
}

/// «A top-up comes → ask → yes: 5 from cash to savings», both devices
/// holding the same vault with the workflow armed.
fn asking_pair() -> (tempfile::TempDir, UnlockedVault, UnlockedVault, String) {
    let (dir, mut left, mut right) = two_devices();
    let cash = left.doc.stores[0].id.clone();
    let savings = store(&mut left, "Savings", StoreKind::Bank, None);
    left.add_transaction(income(&cash, "1000", "opening"))
        .unwrap();
    let id = save(
        &mut left,
        "Ask",
        vec![
            node("t", "trigger.transaction", json!({ "tx_kind": "topup" })),
            node("c", "action.confirm", json!({ "question": "Move 5?" })),
            node(
                "move",
                "action.transaction",
                json!({ "tx_kind": "transfer", "account": { "store": cash }, "to": { "store": savings }, "asset": "USD", "amount": "5" }),
            ),
        ],
        vec![wire("t", "out", "c", "in"), wire("c", "true", "move", "in")],
    );
    settle(&mut left, &mut right);
    left.add_transaction(income(&cash, "50", "salary")).unwrap();
    settle(&mut left, &mut right);
    (dir, left, right, id)
}

fn transfers(vault: &UnlockedVault) -> usize {
    vault
        .doc
        .transactions
        .iter()
        .filter(|tx| {
            matches!(tx, Transaction::Transfer(_))
                && matches!(tx.meta().origin, TxOrigin::Workflow { .. })
        })
        .count()
}

#[test]
fn a_transaction_trigger_on_two_devices_asks_once() {
    let (_dir, mut left, mut right, _id) = asking_pair();
    let now = now_secs() + 5;
    left.run_due_workflows(now, "UTC").unwrap();
    right.run_due_workflows(now, "UTC").unwrap();
    assert_eq!(left.doc.workflow_waits.len(), 1);
    assert_eq!(right.doc.workflow_waits.len(), 1);
    settle(&mut left, &mut right);
    assert_eq!(left.doc.workflow_waits.len(), 1, "two questions");
    assert_eq!(right.doc.workflow_waits, left.doc.workflow_waits);
}

#[test]
fn one_answer_gives_one_set_of_transfers() {
    let (_dir, mut left, mut right, _id) = asking_pair();
    let now = now_secs() + 5;
    left.run_due_workflows(now, "UTC").unwrap();
    right.run_due_workflows(now, "UTC").unwrap();
    // Each device answers the question it has.
    let left_ask = left.doc.workflow_waits[0].id.clone();
    let right_ask = right.doc.workflow_waits[0].id.clone();
    left.answer_workflow_wait(&left_ask, true, now + 1, "UTC")
        .unwrap();
    right
        .answer_workflow_wait(&right_ask, true, now + 1, "UTC")
        .unwrap();
    settle(&mut left, &mut right);
    assert_eq!(transfers(&left), 1, "the answer moved the money twice");
    assert_eq!(transfers(&right), 1);
    assert!(left.doc.workflow_waits.is_empty());
}

#[test]
fn an_answer_whose_result_is_booked_does_nothing() {
    let (_dir, mut left, mut right, _id) = asking_pair();
    let now = now_secs() + 5;
    left.run_due_workflows(now, "UTC").unwrap();
    settle(&mut left, &mut right);
    let ask = right.doc.workflow_waits[0].id.clone();
    // The other device answered; its transfer arrived before the removal did.
    left.answer_workflow_wait(&ask, true, now + 1, "UTC")
        .unwrap();
    let booked: Vec<Transaction> = left
        .doc
        .transactions
        .iter()
        .filter(|tx| {
            matches!(tx.meta().origin, TxOrigin::Workflow { .. })
                && matches!(tx, Transaction::Transfer(_))
        })
        .cloned()
        .collect();
    right.doc.transactions.extend(booked);
    right.persist().unwrap();
    let before = transfers(&right);
    assert!(
        right
            .answer_workflow_wait(&ask, true, now + 2, "UTC")
            .is_err()
    );
    assert_eq!(transfers(&right), before);
    assert!(right.doc.workflow_waits.is_empty());
}

fn old_peer_tx(vault: &mut UnlockedVault, run: &str, key: Option<&str>) {
    let cash = vault.doc.stores[0].id.clone();
    let mut tx = income(&cash, "100", "salary");
    if let Transaction::Topup(topup) = &mut tx {
        topup.meta.origin = TxOrigin::Workflow {
            workflow_id: "salary".into(),
            run_id: run.into(),
            planned_at: Some("2026-11-01T09:00:00Z".into()),
            estimated: false,
            run_key: key.map(str::to_string),
        };
    }
    vault.add_transaction(tx).unwrap();
}

#[test]
fn a_double_run_from_an_old_peer_converges() {
    // The right device is an old build: its operation has no run key.
    let (_dir, mut left, mut right) = two_devices();
    old_peer_tx(
        &mut left,
        "run-left",
        Some("wf:salary:s:2026-11-01T09:00:00Z"),
    );
    std::thread::sleep(std::time::Duration::from_millis(5));
    old_peer_tx(&mut right, "run-right", None);
    for _ in 0..2 {
        exchange(&mut left, &mut right);
    }
    settle(&mut left, &mut right);
    assert_eq!(left.doc.transactions.len(), 1);
    assert_eq!(right.doc.transactions, left.doc.transactions);
}
