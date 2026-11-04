//! What a device writes when a vault opens (125) and the staged exchange
//! queue against what is already known (29): neither may multiply records
//! nor bring back what another device decided.

use super::tests::{settle, two_devices};
use super::*;
use crate::exchange::Execution;
use crate::model::transaction::TxOrigin;
use crate::model::vault::PendingImport;
use crate::service::WorkflowDraft;
use serde_json::json;

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}

fn peer(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

/// `to` takes what `from` has and `to` lacks; nothing flows back.
fn pull_one_way(from: &mut UnlockedVault, to: &mut UnlockedVault) {
    let from_manifest = from.sync_manifest(None).unwrap();
    let to_manifest = to.sync_manifest(None).unwrap();
    let (to_wants, _) = to.sync_diff(&to_manifest, &from_manifest);
    let records = from.sync_records(&to_wants).unwrap();
    to.sync_apply(&records, &peer("from")).unwrap();
}

/// Close the vault and open it again as the same device, as the app does.
fn reopen(mut vault: UnlockedVault, device: &str) -> UnlockedVault {
    vault.persist().unwrap();
    let path = vault.path.clone();
    drop(vault);
    UnlockedVault::open_as(path, "pw", Some(device)).unwrap()
}

const LEFT: &str = "aaaaaaaa-left";
const RIGHT: &str = "bbbbbbbb-right";

// -- 125: the workflow switched off on open -----------------------------------

/// A price watch with no asset: a required field is empty.
fn incomplete_graph() -> String {
    json!({
        "version": 1,
        "nodes": [
            { "id": "t", "kind": "trigger.price",
              "params": { "asset": null, "condition": "above", "value": "1" } },
            { "id": "n", "kind": "action.notify", "params": { "text": "up" } },
        ],
        "edges": [{
            "id": "t.out>n.in", "from": "t", "from_port": "out", "to": "n", "to_port": "in",
        }],
    })
    .to_string()
}

/// Both devices hold the same active workflow that a required field now stops.
fn devices_with_incomplete_workflow() -> (tempfile::TempDir, UnlockedVault, UnlockedVault, String) {
    let (dir, mut left, mut right) = two_devices();
    let saved = left
        .save_workflow(
            None,
            WorkflowDraft {
                name: "Watch".into(),
                description: String::new(),
                active: false,
                graph: incomplete_graph(),
                react_to_workflows: false,
                max_amount: None,
            },
            crate::calendar::now_secs(),
        )
        .unwrap();
    let id = saved.workflow.id;
    left.doc.workflows[0].active = true;
    left.persist().unwrap();
    settle(&mut left, &mut right);
    assert!(right.doc.workflows[0].active);
    (dir, left, right, id)
}

fn off_runs(vault: &UnlockedVault) -> Vec<String> {
    vault
        .doc
        .workflow_runs
        .iter()
        .filter(|run| run.trigger.is_empty())
        .map(|run| run.id.clone())
        .collect()
}

#[test]
fn two_devices_switching_off_one_workflow_leave_one_run_record() {
    let (_dir, left, right, id) = devices_with_incomplete_workflow();
    let mut left = reopen(left, LEFT);
    let mut right = reopen(right, RIGHT);
    assert!(!left.doc.workflows[0].active);
    assert!(!right.doc.workflows[0].active);
    settle(&mut left, &mut right);
    let expected = vec![format!("wf-off-{id}")];
    assert_eq!(off_runs(&left), expected);
    assert_eq!(off_runs(&right), expected);
}

#[test]
fn opening_again_does_not_grow_the_run_list() {
    let (_dir, left, right, id) = devices_with_incomplete_workflow();
    let mut left = reopen(left, LEFT);
    let mut right = reopen(right, RIGHT);
    settle(&mut left, &mut right);
    // The user switches it on again and the field is still empty: the next
    // open replaces the record of the same workflow instead of adding one.
    left.doc.workflows[0].active = true;
    let mut left = reopen(left, LEFT);
    assert!(!left.doc.workflows[0].active);
    assert_eq!(off_runs(&left), vec![format!("wf-off-{id}")]);
    let mut right = reopen(right, RIGHT);
    settle(&mut left, &mut right);
    assert_eq!(off_runs(&right), vec![format!("wf-off-{id}")]);
}

// -- 125: a built-in category deleted elsewhere --------------------------------

fn has_category(vault: &UnlockedVault, key: &str) -> bool {
    vault
        .doc
        .categories
        .iter()
        .any(|category| category.key == key)
}

#[test]
fn a_late_updating_device_does_not_bring_back_a_deleted_builtin_category() {
    let (_dir, mut left, mut right) = two_devices();
    assert!(has_category(&left, "coupons") && has_category(&right, "coupons"));
    // The left device deletes the built-in category; the right one takes the
    // deletion (it holds the tombstone) and is then an older build, which has
    // not seen the batch of categories that brought "coupons".
    left.delete_category("coupons").unwrap();
    settle(&mut left, &mut right);
    assert!(!has_category(&right, "coupons"));
    right.doc.settings.categories_rev = 0;
    pause();
    let mut right = reopen(right, RIGHT);
    assert!(
        !has_category(&right, "coupons"),
        "the opening must not add what a tombstone says is deleted"
    );
    settle(&mut left, &mut right);
    assert!(!has_category(&left, "coupons"));
    assert!(!has_category(&right, "coupons"));
}

// -- 29: the staged queue against what is known --------------------------------

fn staged(store: &str, exec_id: &str) -> PendingImport {
    let fill = Execution {
        exec_id: exec_id.into(),
        symbol: "BTCUSDT".into(),
        side: "buy".into(),
        order_type: "Market".into(),
        exec_price: "65000".into(),
        exec_qty: "0.5".into(),
        exec_value: "32500".into(),
        exec_fee: "0.5".into(),
        exec_time: "1700000000000".into(),
        is_maker: false,
        closed_size: None,
        category: String::new(),
    };
    PendingImport::from_execution(store, "spot", &fill).unwrap()
}

/// An exchange wallet both devices hold.
fn exchange_store(left: &mut UnlockedVault, right: &mut UnlockedVault) -> String {
    let id = left
        .create_store_with(
            "Bybit",
            crate::model::store::StoreKind::Exchange,
            "#f0b90b",
            "",
            None,
            crate::service::StoreExtras {
                provider: Some(crate::model::store::ExchangeProvider::Bybit),
                ..Default::default()
            },
        )
        .unwrap()
        .id;
    settle(left, right);
    id
}

fn pending_ids(vault: &UnlockedVault) -> Vec<String> {
    let mut ids: Vec<String> = vault
        .doc
        .pending_imports
        .iter()
        .map(|pending| pending.exec_id.clone())
        .collect();
    ids.sort();
    ids
}

fn booked(vault: &UnlockedVault, exec_id: &str) -> usize {
    vault
        .doc
        .transactions
        .iter()
        .filter(|tx| matches!(&tx.meta().origin, TxOrigin::Import { exec_id: id } if id == exec_id))
        .count()
}

#[test]
fn a_queue_entry_for_a_booked_import_does_not_come_back() {
    let (_dir, mut left, mut right) = two_devices();
    let store = exchange_store(&mut left, &mut right);
    left.stage_imports(vec![staged(&store, "e1")]).unwrap();
    left.confirm_pending_imports(&[]).unwrap();
    pause();
    // The right device has not seen the booking and queues the same fill.
    right.stage_imports(vec![staged(&store, "e1")]).unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert!(pending_ids(device).is_empty(), "{:?}", pending_ids(device));
        assert_eq!(booked(device, "e1"), 1);
    }
}

#[test]
fn a_queue_entry_for_a_discarded_import_does_not_come_back() {
    let (_dir, mut left, mut right) = two_devices();
    let store = exchange_store(&mut left, &mut right);
    left.stage_imports(vec![staged(&store, "e1")]).unwrap();
    left.discard_pending_imports(&[]).unwrap();
    pause();
    right.stage_imports(vec![staged(&store, "e1")]).unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert!(pending_ids(device).is_empty(), "{:?}", pending_ids(device));
        assert!(device.doc.discarded_imports.contains(&"e1".to_string()));
    }
}

#[test]
fn a_truly_pending_import_survives_the_rule() {
    let (_dir, mut left, mut right) = two_devices();
    let store = exchange_store(&mut left, &mut right);
    // One fill is booked on the left device before the right one queues it.
    left.stage_imports(vec![staged(&store, "e1")]).unwrap();
    left.confirm_pending_imports(&[]).unwrap();
    pause();
    right
        .stage_imports(vec![staged(&store, "e1"), staged(&store, "e2")])
        .unwrap();
    // Nothing is known on the right yet: the pass leaves the whole queue alone.
    let report = right.repair_invariants(None).unwrap();
    assert!(report.dropped_imports.is_empty());
    assert_eq!(
        pending_ids(&right),
        vec!["e1".to_string(), "e2".to_string()]
    );
    // After the exchange only the booked one goes; the truly pending stays.
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert_eq!(pending_ids(device), vec!["e2".to_string()]);
        assert_eq!(booked(device, "e1"), 1);
    }
}

#[test]
fn the_queue_rule_is_idempotent_and_undone_with_its_batch() {
    let (_dir, mut left, mut right) = two_devices();
    let store = exchange_store(&mut left, &mut right);
    left.stage_imports(vec![staged(&store, "e1")]).unwrap();
    left.confirm_pending_imports(&[]).unwrap();
    right.stage_imports(vec![staged(&store, "e1")]).unwrap();
    let before = right.doc.sync.batches.len();
    pull_one_way(&mut left, &mut right);
    assert_eq!(right.doc.sync.batches.len(), before + 1);
    assert!(pending_ids(&right).is_empty());
    assert_eq!(booked(&right, "e1"), 1);
    // A second pass finds nothing to do.
    let doc = right.doc_value().unwrap();
    let report = right.repair_invariants(None).unwrap();
    assert!(report.dropped_imports.is_empty());
    assert_eq!(right.doc_value().unwrap(), doc);
    // Undoing the batch returns the entry (and removes the booking again).
    let batch = right.doc.sync.batches.last().unwrap().id.clone();
    right.sync_undo(&batch).unwrap();
    assert_eq!(pending_ids(&right), vec!["e1".to_string()]);
    assert_eq!(booked(&right, "e1"), 0);
}
