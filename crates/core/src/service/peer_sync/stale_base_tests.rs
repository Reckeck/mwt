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
                max_amount_currency: None,
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

// -- 99: a whole edited document against the base the user saw ------------------

fn new_contact(vault: &mut UnlockedVault, name: &str) -> String {
    vault
        .create_contact(crate::service::contacts::ContactDraft {
            name: name.into(),
            kind: crate::model::contact::ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        })
        .unwrap()
        .id
}

fn contact_names(vault: &UnlockedVault) -> Vec<String> {
    let mut names: Vec<String> = vault
        .doc
        .contacts
        .iter()
        .map(|contact| contact.name.clone())
        .collect();
    names.sort();
    names
}

fn edit_note(doc: &mut VaultDoc, name: &str, note: &str) {
    doc.contacts
        .iter_mut()
        .find(|contact| contact.name == name)
        .unwrap()
        .note = note.into();
}

#[test]
fn a_record_that_arrives_while_editing_survives_the_save() {
    let (_dir, mut left, mut right) = two_devices();
    new_contact(&mut left, "Mine");
    settle(&mut left, &mut right);
    let base = left.doc.clone();
    // The peer adds a record while the editor is open.
    let theirs = new_contact(&mut right, "Theirs");
    settle(&mut left, &mut right);
    let mut edited = base.clone();
    edit_note(&mut edited, "Mine", "edited");
    left.apply_document_edit(&base, edited).unwrap();

    assert_eq!(contact_names(&left), vec!["Mine", "Theirs"]);
    assert!(
        !left
            .doc
            .sync
            .tombstones
            .contains_key(&format!("contacts:{theirs}"))
    );
    settle(&mut left, &mut right);
    assert_eq!(contact_names(&right), vec!["Mine", "Theirs"]);
    assert_eq!(
        right
            .doc
            .contacts
            .iter()
            .find(|contact| contact.name == "Mine")
            .unwrap()
            .note,
        "edited"
    );
    assert!(right.doc.sync.tombstones.is_empty());
}

#[test]
fn a_record_deleted_in_the_editor_gets_a_tombstone_and_only_it() {
    let (_dir, mut left, mut right) = two_devices();
    let gone = new_contact(&mut left, "Gone");
    new_contact(&mut left, "Kept");
    settle(&mut left, &mut right);
    let base = left.doc.clone();
    new_contact(&mut right, "Late");
    settle(&mut left, &mut right);
    let mut edited = base.clone();
    edited.contacts.retain(|contact| contact.name != "Gone");
    left.apply_document_edit(&base, edited).unwrap();

    let tombstoned: Vec<&String> = left.doc.sync.tombstones.keys().collect();
    assert_eq!(tombstoned, vec![&format!("contacts:{gone}")]);
    assert_eq!(contact_names(&left), vec!["Kept", "Late"]);
    settle(&mut left, &mut right);
    assert_eq!(contact_names(&right), vec!["Kept", "Late"]);
}

#[test]
fn replacing_without_a_base_still_rolls_back_what_arrived_meanwhile() {
    let (_dir, mut left, mut right) = two_devices();
    let base = left.doc.clone();
    new_contact(&mut right, "Theirs");
    settle(&mut left, &mut right);
    left.replace_doc(base).unwrap();
    assert!(contact_names(&left).is_empty());
}

#[test]
fn an_edit_equal_to_its_base_stamps_nothing() {
    let (_dir, mut left, mut right) = two_devices();
    new_contact(&mut left, "Mine");
    settle(&mut left, &mut right);
    let base = left.doc.clone();
    let stamps = left.doc.sync.stamps.clone();
    let tombstones = left.doc.sync.tombstones.clone();
    left.apply_document_edit(&base, base.clone()).unwrap();
    assert_eq!(left.doc.sync.stamps, stamps);
    assert_eq!(left.doc.sync.tombstones, tombstones);
    assert_eq!(left.doc.contacts, base.contacts);
}

#[test]
fn a_money_edit_recomputes_holdings_and_pins_identity() {
    use crate::service::test_support::{amount, income, usd};
    let (_dir, mut left, _right) = two_devices();
    let cash = left.doc.stores[0].id.clone();
    left.add_transaction(income(&cash, "100", "pay")).unwrap();
    let base = left.doc.clone();
    let mut edited = base.clone();
    let crate::model::transaction::Transaction::Topup(topup) = &mut edited.transactions[0] else {
        panic!("a top-up")
    };
    topup.amount = amount("250");
    edited.holdings.clear();
    edited.id = "forged".into();
    edited.version = 99;
    left.apply_document_edit(&base, edited).unwrap();

    let held = left
        .doc
        .holdings
        .iter()
        .find(|holding| holding.store_id == cash && holding.asset == usd())
        .unwrap();
    assert_eq!(held.quantity.to_string(), "250");
    assert_eq!(left.doc.id, base.id);
    assert_eq!(left.doc.version, crate::VAULT_FORMAT_VERSION);
}

#[test]
fn a_device_setting_changed_in_the_editor_is_applied() {
    let (_dir, mut left, _right) = two_devices();
    let base = left.doc.clone();
    let mut edited = base.clone();
    edited.settings.minimize_to_tray = !base.settings.minimize_to_tray;
    left.apply_document_edit(&base, edited).unwrap();
    assert_eq!(
        left.doc.settings.minimize_to_tray,
        !base.settings.minimize_to_tray
    );
}

#[test]
fn a_setting_changed_by_the_editor_does_not_undo_a_peers_other_setting() {
    let (_dir, mut left, _right) = two_devices();
    let base = left.doc.clone();
    left.doc.settings.budget_currency = "eur".into();
    left.persist().unwrap();
    let mut edited = base.clone();
    edited.settings.minimize_to_tray = !base.settings.minimize_to_tray;
    left.apply_document_edit(&base, edited).unwrap();
    assert_eq!(left.doc.settings.budget_currency, "eur");
}

// -- 99: restoring a copy in a group ------------------------------------------

fn live_peer(vault: &mut UnlockedVault) {
    vault.sync_trust_peer("dev-b", "Phone", "aa").unwrap();
}

#[test]
fn restoring_with_peers_returns_the_copys_records_and_deletes_nothing() {
    let (_dir, mut left, mut right) = two_devices();
    let kept = new_contact(&mut left, "Old name");
    settle(&mut left, &mut right);
    let backup = left.create_backup().unwrap();
    edit_note(&mut left.doc, "Old name", "changed after the copy");
    left.doc.contacts[0].name = "New name".into();
    left.persist().unwrap();
    new_contact(&mut right, "Newer");
    settle(&mut left, &mut right);
    live_peer(&mut left);

    left.restore_backup(&backup.name).unwrap();
    assert_eq!(contact_names(&left), vec!["Newer", "Old name"]);
    assert!(left.doc.sync.tombstones.is_empty());
    settle(&mut left, &mut right);
    assert_eq!(contact_names(&right), vec!["Newer", "Old name"]);
    assert!(right.doc.contacts.iter().any(|contact| contact.id == kept));
}

#[test]
fn restoring_with_peers_brings_back_a_deleted_record_to_the_peer() {
    let (_dir, mut left, mut right) = two_devices();
    let id = new_contact(&mut left, "Doomed");
    settle(&mut left, &mut right);
    let backup = left.create_backup().unwrap();
    left.delete_contact(&id).unwrap();
    settle(&mut left, &mut right);
    assert!(right.doc.contacts.is_empty());
    assert!(
        left.doc
            .sync
            .tombstones
            .contains_key(&format!("contacts:{id}"))
    );
    live_peer(&mut left);

    left.restore_backup(&backup.name).unwrap();
    assert_eq!(contact_names(&left), vec!["Doomed"]);
    assert!(
        !left
            .doc
            .sync
            .tombstones
            .contains_key(&format!("contacts:{id}"))
    );
    settle(&mut left, &mut right);
    assert_eq!(contact_names(&right), vec!["Doomed"]);
    assert!(
        !right
            .doc
            .sync
            .tombstones
            .contains_key(&format!("contacts:{id}"))
    );
}

#[test]
fn restoring_after_the_only_peer_was_forgotten_replaces_as_before() {
    let (_dir, mut left, mut right) = two_devices();
    live_peer(&mut left);
    left.sync_forget_peer("dev-b").unwrap();
    let backup = left.create_backup().unwrap();
    new_contact(&mut left, "Later");
    settle(&mut left, &mut right);
    left.restore_backup(&backup.name).unwrap();
    assert!(contact_names(&left).is_empty());
}

#[test]
fn an_exchange_terminal_edited_in_the_editor_is_applied_and_one_left_alone_is_kept() {
    use crate::model::store::{ExchangeProvider, StoreKind};
    use crate::service::{StoreExtras, TerminalPatch};
    let (_dir, mut left, _right) = two_devices();
    let patch = |timeframe: &str| TerminalPatch {
        timeframe: Some(timeframe.into()),
        ..TerminalPatch::default()
    };
    let id = left
        .create_store_with(
            "Bybit",
            StoreKind::Exchange,
            "",
            "",
            None,
            StoreExtras {
                provider: Some(ExchangeProvider::Bybit),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    left.update_terminal_state(&id, patch("1h")).unwrap();
    let base = left.doc.clone();
    // The terminal moves on after the buffer was loaded; an edit elsewhere
    // must not roll it back.
    left.update_terminal_state(&id, patch("4h")).unwrap();
    let mut edited = base.clone();
    edited
        .stores
        .iter_mut()
        .find(|store| store.id == id)
        .unwrap()
        .name = "Renamed".into();
    left.apply_document_edit(&base, edited).unwrap();
    assert_eq!(left.terminal_state(&id).unwrap().timeframe, "4h");

    // An edit of the terminal itself is applied.
    let base = left.doc.clone();
    let mut edited = base.clone();
    let json = serde_json::to_string(&edited)
        .unwrap()
        .replace("\"4h\"", "\"1d\"");
    edited = serde_json::from_str(&json).unwrap();
    left.apply_document_edit(&base, edited).unwrap();
    assert_eq!(left.terminal_state(&id).unwrap().timeframe, "1d");
}
