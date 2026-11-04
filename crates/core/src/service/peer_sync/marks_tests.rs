//! Trigger marks merge entry by entry, a schedule mark never goes back past a
//! booked moment, the transaction cursor stays on its device, and an undo that
//! restored nothing stays available.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::calendar::now_secs;
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::pricing::history::unix_secs;
use crate::service::test_support::{income, store};
use crate::service::workflows::WorkflowDraft;
use crate::sync::stamp::now_ms;

fn peer(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

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

fn state_key(workflow_id: &str) -> String {
    format!("workflow_state:{workflow_id}")
}

/// A workflow on both devices with an (empty) state record on both.
fn shared_workflow() -> (tempfile::TempDir, UnlockedVault, UnlockedVault, String) {
    let (dir, mut left, mut right) = two_devices();
    let id = save(
        &mut left,
        "Marks",
        vec![node("t", "trigger.manual", json!({}))],
        vec![],
    );
    left.wf_set_mark(&id, "schedule:base", "2026-03-01T00:00:00Z");
    left.persist().unwrap();
    settle(&mut left, &mut right);
    (dir, left, right, id)
}

fn mark_of(vault: &UnlockedVault, id: &str, key: &str) -> Option<String> {
    vault.wf_mark(id, key)
}

#[test]
fn marks_of_two_triggers_both_survive_a_merge() {
    let (_dir, mut left, mut right, id) = shared_workflow();
    left.wf_set_mark(&id, "schedule:a", "2026-03-02T09:00:00Z");
    left.persist().unwrap();
    right.wf_set_mark(&id, "threshold:b", "above");
    right.persist().unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(
            mark_of(vault, &id, "schedule:a").as_deref(),
            Some("2026-03-02T09:00:00Z")
        );
        assert_eq!(mark_of(vault, &id, "threshold:b").as_deref(), Some("above"));
        assert_eq!(
            mark_of(vault, &id, "schedule:base").as_deref(),
            Some("2026-03-01T00:00:00Z")
        );
    }
}

#[test]
fn marks_from_an_old_peer_do_not_erase_another_trigger() {
    let (_dir, mut left, mut right, id) = shared_workflow();
    right.wf_set_mark(&id, "threshold:b", "above");
    right.persist().unwrap();
    // What an old device sends: the whole map as one field, one stamp, and
    // only the entries it knows.
    let key = state_key(&id);
    let mut incoming = left.sync_records(std::slice::from_ref(&key)).unwrap();
    let state = incoming.get_mut(&key).unwrap();
    let later = Stamp::new(now_ms() + 1_000, 0, "oldpeer");
    state.value = Some(json!({
        "workflow_id": id,
        "marks": { "schedule:base": "2026-03-05T00:00:00Z" },
        "tx_cursor": "from-the-old-peer",
        "tx_index": 99,
    }));
    let stamps = state.stamps.as_mut().unwrap();
    stamps.fields.clear();
    stamps.fields.insert("marks".into(), later.clone());
    stamps.fields.insert("tx_cursor".into(), later);
    right.sync_apply(&incoming, &peer("old")).unwrap();
    assert_eq!(
        mark_of(&right, &id, "schedule:base").as_deref(),
        Some("2026-03-05T00:00:00Z"),
        "the old peer's newer entry is taken"
    );
    assert_eq!(
        mark_of(&right, &id, "threshold:b").as_deref(),
        Some("above"),
        "an entry the old map lacks was dropped"
    );
    // The new side still sends the full map, and a stamp an old peer reads.
    let sent = right.sync_records(std::slice::from_ref(&key)).unwrap();
    let sent = &sent[&key];
    let marks = sent.value.as_ref().unwrap()["marks"].as_object().unwrap();
    assert_eq!(marks.len(), 2, "{marks:?}");
    assert!(sent.stamps.as_ref().unwrap().fields.contains_key("marks"));
}

/// «A top-up comes → move 5 from cash to savings», armed on both devices.
fn moving_pair() -> (
    tempfile::TempDir,
    UnlockedVault,
    UnlockedVault,
    String,
    String,
) {
    let (dir, mut left, mut right) = two_devices();
    let cash = left.doc.stores[0].id.clone();
    let savings = store(&mut left, "Savings", StoreKind::Bank, None);
    left.add_transaction(income(&cash, "1000", "opening"))
        .unwrap();
    let id = save(
        &mut left,
        "Move",
        vec![
            node("t", "trigger.transaction", json!({ "tx_kind": "topup" })),
            node(
                "move",
                "action.transaction",
                json!({ "tx_kind": "transfer", "account": { "store": cash }, "to": { "store": savings }, "asset": "USD", "amount": "5" }),
            ),
        ],
        vec![wire("t", "out", "move", "in")],
    );
    settle(&mut left, &mut right);
    (dir, left, right, id, cash)
}

fn cursor_of(vault: &UnlockedVault, id: &str) -> (Option<String>, usize) {
    let state = vault
        .doc
        .workflow_state
        .iter()
        .find(|state| state.workflow_id == id)
        .unwrap();
    (state.tx_cursor.clone(), state.tx_index)
}

#[test]
fn the_transaction_cursor_is_not_synced() {
    let (_dir, mut left, mut right, id, cash) = moving_pair();
    let right_before = cursor_of(&right, &id);
    left.add_transaction(income(&cash, "50", "salary")).unwrap();
    left.run_due_workflows(now_secs() + 5, "UTC").unwrap();
    let left_after = cursor_of(&left, &id);
    assert_ne!(left_after, right_before);
    settle(&mut left, &mut right);
    assert_eq!(cursor_of(&right, &id), right_before, "the cursor travelled");
    assert_eq!(cursor_of(&left, &id), left_after);

    // An old peer still sends its cursor: it is ignored.
    let key = state_key(&id);
    let mut incoming = left.sync_records(std::slice::from_ref(&key)).unwrap();
    let state = incoming.get_mut(&key).unwrap();
    let mut value = state.value.clone().unwrap();
    value["tx_cursor"] = json!("from-the-old-peer");
    value["tx_index"] = json!(77);
    value["marks"]["schedule:new"] = json!("2026-03-09T00:00:00Z");
    state.value = Some(value);
    state
        .stamps
        .as_mut()
        .unwrap()
        .fields
        .insert("marks".into(), Stamp::new(now_ms() + 1_000, 0, "oldpeer"));
    right.sync_apply(&incoming, &peer("old")).unwrap();
    assert_eq!(cursor_of(&right, &id), right_before);
}

#[test]
fn a_different_cursor_does_not_post_twice() {
    let (_dir, mut left, mut right, id, cash) = moving_pair();
    left.add_transaction(income(&cash, "50", "salary")).unwrap();
    settle(&mut left, &mut right);
    let now = now_secs() + 5;
    left.run_due_workflows(now, "UTC").unwrap();
    settle(&mut left, &mut right);
    // The right device never looked at the salary: its cursor is older.
    right.run_due_workflows(now, "UTC").unwrap();
    settle(&mut left, &mut right);
    let moved = |vault: &UnlockedVault| {
        vault
            .doc
            .transactions
            .iter()
            .filter(|tx| {
                matches!(tx, Transaction::Transfer(_))
                    && matches!(tx.meta().origin, TxOrigin::Workflow { .. })
            })
            .count()
    };
    assert_eq!(moved(&left), 1, "{}", id);
    assert_eq!(moved(&right), 1);
}

/// «Every day at 09:00 → top up cash by 10», two moments booked on the left
/// and carried to the right in separate exchanges.
fn two_moments() -> (
    tempfile::TempDir,
    UnlockedVault,
    UnlockedVault,
    String,
    String,
) {
    let (dir, mut left, mut right) = two_devices();
    let cash = left.doc.stores[0].id.clone();
    let id = save(
        &mut left,
        "Daily",
        vec![
            node(
                "s",
                "trigger.schedule",
                json!({ "every": "day", "time": "09:00" }),
            ),
            node(
                "post",
                "action.transaction",
                json!({ "tx_kind": "topup", "asset": "USD", "amount": "10", "category": "salary", "account": { "store": cash } }),
            ),
        ],
        vec![wire("s", "out", "post", "in")],
    );
    left.wf_set_mark(&id, "schedule:s", "2026-03-01T10:00:00Z");
    left.persist().unwrap();
    settle(&mut left, &mut right);
    left.run_due_workflows(at("2026-03-02T10:00:00Z"), "UTC")
        .unwrap();
    settle(&mut left, &mut right);
    left.run_due_workflows(at("2026-03-03T10:00:00Z"), "UTC")
        .unwrap();
    let newest = mark_of(&left, &id, "schedule:s").unwrap();
    (dir, left, right, id, newest)
}

#[test]
fn undo_does_not_move_a_schedule_mark_back() {
    let (_dir, mut left, mut right, id, newest) = two_moments();
    let (_, applied) = exchange(&mut left, &mut right);
    let batch = applied.expect("the right device changed");
    assert_eq!(mark_of(&right, &id, "schedule:s"), Some(newest.clone()));
    // The newest operation is edited here, so the undo leaves it booked.
    for tx in &mut right.doc.transactions {
        if let Transaction::Topup(topup) = tx
            && matches!(&topup.meta.origin, TxOrigin::Workflow { run_key: Some(key), .. } if key.ends_with(&newest))
        {
            topup.meta.comment = "mine now".into();
        }
    }
    right.persist().unwrap();
    let report = right.sync_undo(&batch.id).unwrap();
    assert_eq!(report.skipped.len(), 1, "{report:?}");
    assert_eq!(
        mark_of(&right, &id, "schedule:s"),
        Some(newest),
        "the mark went back below a booked moment"
    );
}

#[test]
fn undoing_an_operation_here_does_not_move_a_schedule_mark_back() {
    let (_dir, mut left, _right, id, newest) = two_moments();
    let booked: Vec<String> = left
        .doc
        .transactions
        .iter()
        .map(|tx| tx.meta().id.clone())
        .collect();
    for tx_id in booked {
        left.delete_transaction(&tx_id).unwrap();
    }
    assert_eq!(mark_of(&left, &id, "schedule:s"), Some(newest));
    let pass = left
        .run_due_workflows(at("2026-03-03T10:00:00Z"), "UTC")
        .unwrap();
    assert!(pass.created.is_empty(), "{:?}", pass.created);
}

#[test]
fn a_batch_with_nothing_restored_stays_undoable() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = left.doc.stores[0].id.clone();
    left.add_transaction(income(&cash, "20", "theirs")).unwrap();
    let (_, applied) = exchange(&mut left, &mut right);
    let batch = applied.expect("the right device changed");
    // Everything the batch brought is changed here since.
    for tx in &mut right.doc.transactions {
        if let Transaction::Topup(topup) = tx {
            topup.meta.comment = "edited here".into();
        }
    }
    right.doc.settings.schedule_timezone = Some("Asia/Bangkok".into());
    right.persist().unwrap();
    let report = right.sync_undo(&batch.id).unwrap();
    assert_eq!(report.restored, 0);
    assert_eq!(report.skipped.len(), 2, "{report:?}");
    let listed: BTreeMap<_, _> = right
        .sync_history()
        .into_iter()
        .map(|entry| (entry.id.clone(), entry.undone_at))
        .collect();
    assert_eq!(listed[&batch.id], None, "the batch was marked undone");
    // It can still be tried again.
    assert!(right.sync_undo(&batch.id).is_ok());
}
