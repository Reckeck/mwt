//! R3: a record that live money points to comes back after a parallel delete.

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::ipc_contract::{SharedEventDraft, SharedParticipantDraft, SharedPurchaseDraft};
use crate::model::category::CategoryGroup;
use crate::model::contact::ContactKind;
use crate::model::shared_event::PriceMode;
use crate::model::store::StoreKind;
use crate::service::contacts::ContactDraft;
use crate::service::test_support::{amount, income, store};

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(3));
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

fn contact(vault: &mut UnlockedVault, name: &str) -> String {
    vault
        .create_contact(ContactDraft {
            name: name.into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        })
        .unwrap()
        .id
}

fn categories_of(vault: &UnlockedVault, key: &str) -> Vec<crate::model::category::Category> {
    vault
        .doc
        .categories
        .iter()
        .filter(|category| category.key == key)
        .cloned()
        .collect()
}

fn pay_with_category(vault: &mut UnlockedVault, wallet: &str, key: &str) {
    let mut tx = income(wallet, "10", "uses it");
    tx.meta_mut().categories = vec![key.to_string()];
    vault.add_transaction(tx).unwrap();
}

#[test]
fn a_category_deleted_while_an_operation_uses_it_comes_back_on_both_devices() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let key = left
        .create_category(None, "Hobby", CategoryGroup::Income)
        .unwrap()
        .key;
    settle(&mut left, &mut right);
    left.delete_category(&key).unwrap();
    pause();
    pay_with_category(&mut right, &wallet, &key);
    settle(&mut left, &mut right);
    assert_eq!(categories_of(&left, &key).len(), 1);
    assert_eq!(categories_of(&left, &key), categories_of(&right, &key));
    assert_eq!(categories_of(&left, &key)[0].name.as_deref(), Some("Hobby"));
    assert!(!categories_of(&left, &key)[0].archived);
    // Nobody keeps repairing: one more exchange moves nothing.
    assert_eq!(exchange(&mut left, &mut right), (None, None));
}

#[test]
fn a_sub_account_deleted_while_an_operation_uses_it_comes_back() {
    let (_dir, mut left, mut right) = two_devices();
    let bank = store(&mut left, "bank", StoreKind::Bank, None);
    let savings = store(&mut left, "savings", StoreKind::Bank, Some(&bank));
    settle(&mut left, &mut right);
    left.delete_store(&savings).unwrap();
    pause();
    right.add_transaction(income(&savings, "10", "in")).unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let node = device.find_store(&savings).expect("the account is back");
        assert_eq!(node.name, "savings");
        assert_eq!(node.parent_id.as_deref(), Some(bank.as_str()));
        assert!(!node.archived);
    }
    assert_eq!(left.find_store(&savings), right.find_store(&savings));
}

#[test]
fn a_contact_deleted_while_an_operation_names_it_comes_back() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let friend = contact(&mut left, "Friend");
    settle(&mut left, &mut right);
    left.delete_contact(&friend).unwrap();
    pause();
    let mut tx = income(&wallet, "10", "from a friend");
    tx.meta_mut().contact_id = Some(friend.clone());
    right.add_transaction(tx).unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let found: Vec<_> = device
            .doc
            .contacts
            .iter()
            .filter(|row| row.id == friend)
            .collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Friend");
    }
}

#[test]
fn a_participant_removed_while_a_purchase_names_them_comes_back() {
    let (_dir, mut left, mut right) = two_devices();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "Trip".into(),
            currency: "usd".into(),
            i_participate: true,
            participants: vec![SharedParticipantDraft {
                id: Some("olya".into()),
                name: "Olya".into(),
                ..Default::default()
            }],
            ..SharedEventDraft::default()
        })
        .unwrap()
        .event
        .id;
    settle(&mut left, &mut right);
    left.remove_shared_participant(&event, "olya").unwrap();
    pause();
    right
        .add_shared_purchase(
            &event,
            SharedPurchaseDraft {
                new_item_title: Some("Taxi".into()),
                buyer: "olya".into(),
                qty: amount("1"),
                amount: amount("20"),
                currency: "usd".into(),
                price_mode: PriceMode::Total,
                at: Some("2026-10-03".into()),
                ..Default::default()
            },
        )
        .unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let found = &device.doc.shared_events[0];
        assert_eq!(found.participants.len(), 1);
        assert_eq!(found.participants[0].name, "Olya");
    }
    assert_eq!(left.doc.shared_events, right.doc.shared_events);
}

#[test]
fn a_device_without_the_previous_value_keeps_the_reference_and_invents_nothing() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    // Made and deleted on the left: no batch ever carried its value there.
    let key = left
        .create_category(None, "Hobby", CategoryGroup::Income)
        .unwrap()
        .key;
    settle(&mut left, &mut right);
    left.delete_category(&key).unwrap();
    pause();
    pay_with_category(&mut right, &wallet, &key);
    pull_one_way(&mut right, &mut left);
    assert!(categories_of(&left, &key).is_empty(), "no stub record");
    assert!(
        left.doc
            .transactions
            .iter()
            .any(|tx| tx.meta().categories == vec![key.clone()]),
        "the reference stays"
    );
    let report = left.repair_invariants(None).unwrap();
    assert_eq!(report.unrestored, vec![key.clone()]);
    assert!(report.restored.is_empty());
    // The right device has the value; its restore arrives by sync.
    settle(&mut left, &mut right);
    assert_eq!(categories_of(&left, &key).len(), 1);
    assert_eq!(categories_of(&left, &key), categories_of(&right, &key));
}

#[test]
fn a_second_pass_restores_nothing_more() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let key = left
        .create_category(None, "Hobby", CategoryGroup::Income)
        .unwrap()
        .key;
    settle(&mut left, &mut right);
    left.delete_category(&key).unwrap();
    pause();
    pay_with_category(&mut right, &wallet, &key);
    // The right device restores in this exchange; a later pass must not stamp again.
    settle(&mut left, &mut right);
    assert!(
        right
            .doc
            .notification_log
            .iter()
            .any(|record| record.key.starts_with("repair:restored:")),
        "this device did restore"
    );
    let stamps = right.doc.sync.stamps.clone();
    let tombstones = right.doc.sync.tombstones.clone();
    let log = right.doc.notification_log.len();
    assert!(right.repair_invariants(None).unwrap().is_empty());
    assert_eq!(right.doc.sync.stamps, stamps);
    assert_eq!(right.doc.sync.tombstones, tombstones);
    assert_eq!(right.doc.notification_log.len(), log);
}

#[test]
fn the_restore_is_told_to_the_user() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let key = left
        .create_category(None, "Hobby", CategoryGroup::Income)
        .unwrap()
        .key;
    settle(&mut left, &mut right);
    left.delete_category(&key).unwrap();
    pause();
    pay_with_category(&mut right, &wallet, &key);
    settle(&mut left, &mut right);
    let notice = right
        .doc
        .notification_log
        .iter()
        .find(|record| record.key.starts_with("repair:restored:"))
        .expect("a notice");
    assert!(notice.body.contains("Hobby"), "{}", notice.body);
    assert!(notice.body.contains("operations still use them") || notice.body.contains("ссылаются"));
}

/// A third device sharing the vault of `left` as it is now.
fn third_device(dir: &tempfile::TempDir, left: &UnlockedVault) -> UnlockedVault {
    let copy = dir.path().join("third.wallet");
    std::fs::copy(&left.path, &copy).unwrap();
    let mut third = UnlockedVault::open(copy, "pw").unwrap();
    third.set_device("cccccccc-third");
    third
}

fn rename(vault: &mut UnlockedVault, key: &str, name: &str) {
    vault
        .update_category(key, Some(name), CategoryGroup::Income, None, None)
        .unwrap();
}

#[test]
fn a_rename_then_a_local_delete_never_brings_the_old_name_back() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let key = left
        .create_category(None, "Hobby", CategoryGroup::Income)
        .unwrap()
        .key;
    settle(&mut left, &mut right);
    pause();
    rename(&mut right, &key, "Hobby 2");
    settle(&mut left, &mut right);
    // The left took the rename in a batch, then deletes the category itself.
    left.delete_category(&key).unwrap();
    pause();
    pay_with_category(&mut right, &wallet, &key);
    // The deleting device has no batch for its own deletion: nothing to restore.
    pull_one_way(&mut right, &mut left);
    assert!(
        categories_of(&left, &key).is_empty(),
        "no stale value invented"
    );
    let report = left.repair_invariants(None).unwrap();
    assert_eq!(report.unrestored, vec![key.clone()]);
    // The right device took the deletion in a batch and restores the latest value.
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert_eq!(categories_of(device, &key).len(), 1);
        assert_eq!(
            categories_of(device, &key)[0].name.as_deref(),
            Some("Hobby 2")
        );
    }
}

#[test]
fn undoing_the_batch_removes_the_restore_again() {
    let (dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let key = left
        .create_category(None, "Hobby", CategoryGroup::Income)
        .unwrap()
        .key;
    settle(&mut left, &mut right);
    let mut third = third_device(&dir, &left);
    pause();
    rename(&mut right, &key, "Hobby 2");
    settle(&mut left, &mut right);
    right.delete_category(&key).unwrap();
    // The left takes the deletion in a batch: the last value it had is "Hobby 2".
    settle(&mut left, &mut right);
    assert!(categories_of(&left, &key).is_empty());
    pause();
    // The third device never saw the deletion and books an operation with it.
    pay_with_category(&mut third, &wallet, &key);
    let before = left.doc.sync.batches.len();
    pull_one_way(&mut third, &mut left);
    assert_eq!(left.doc.sync.batches.len(), before + 1);
    assert_eq!(categories_of(&left, &key).len(), 1);
    assert_eq!(
        categories_of(&left, &key)[0].name.as_deref(),
        Some("Hobby 2")
    );
    let batch = left.doc.sync.batches.last().unwrap().id.clone();
    assert!(left.sync_undo(&batch).unwrap().restored > 0);
    assert!(
        categories_of(&left, &key).is_empty(),
        "the restore is undone"
    );
}
