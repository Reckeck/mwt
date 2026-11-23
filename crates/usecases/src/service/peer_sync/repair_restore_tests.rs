//! R3: a record that live money points to comes back after a parallel delete.

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::household::contacts::ContactDraft;
use crate::ipc_contract::{SharedEventDraft, SharedParticipantDraft, SharedPurchaseDraft};
use crate::model::category::CategoryGroup;
use crate::model::contact::ContactKind;
use crate::model::shared_event::PriceMode;
use crate::model::store::StoreKind;
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
fn pull_one_way(from: &mut Session, to: &mut Session) {
    let from_manifest = crate::usecases::sync::read::sync_manifest(from, None).unwrap();
    let to_manifest = crate::usecases::sync::read::sync_manifest(to, None).unwrap();
    let (to_wants, _) = crate::usecases::sync::read::sync_diff(to, &to_manifest, &from_manifest);
    let records = crate::usecases::sync::read::sync_records(from, &to_wants).unwrap();
    peer_sync::sync_apply(to, &records, &peer("from")).unwrap();
}

fn contact(vault: &mut Session, name: &str) -> String {
    crate::usecases::household::contacts::create_contact(
        vault,
        ContactDraft {
            name: name.into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        },
    )
    .unwrap()
    .id
}

fn categories_of(vault: &Session, key: &str) -> Vec<crate::model::category::Category> {
    vault
        .doc
        .categories
        .iter()
        .filter(|category| category.key == key)
        .cloned()
        .collect()
}

fn pay_with_category(vault: &mut Session, wallet: &str, key: &str) {
    let mut tx = income(wallet, "10", "uses it");
    tx.meta_mut().categories = vec![key.to_string()];
    crate::usecases::money::transactions::add_transaction(vault, tx).unwrap();
}

#[test]
fn a_category_deleted_while_an_operation_uses_it_comes_back_on_both_devices() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let key = crate::usecases::household::categories::create_category(
        &mut left,
        None,
        "Hobby",
        CategoryGroup::Income,
    )
    .unwrap()
    .key;
    settle(&mut left, &mut right);
    crate::usecases::household::categories::delete_category(&mut left, &key).unwrap();
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
    crate::usecases::money::stores::delete_store(&mut left, &savings).unwrap();
    pause();
    crate::usecases::money::transactions::add_transaction(&mut right, income(&savings, "10", "in"))
        .unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let node =
            crate::money::tree::find_store(&device.doc, &savings).expect("the account is back");
        assert_eq!(node.name, "savings");
        assert_eq!(node.parent_id.as_deref(), Some(bank.as_str()));
        assert!(!node.archived);
    }
    assert_eq!(
        crate::money::tree::find_store(&left.doc, &savings),
        crate::money::tree::find_store(&right.doc, &savings)
    );
}

#[test]
fn a_contact_deleted_while_an_operation_names_it_comes_back() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let friend = contact(&mut left, "Friend");
    settle(&mut left, &mut right);
    crate::usecases::household::contacts::delete_contact(&mut left, &friend).unwrap();
    pause();
    let mut tx = income(&wallet, "10", "from a friend");
    tx.meta_mut().contact_id = Some(friend.clone());
    crate::usecases::money::transactions::add_transaction(&mut right, tx).unwrap();
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
    let event = crate::usecases::household::shared_events::create_shared_event(
        &mut left,
        SharedEventDraft {
            title: "Trip".into(),
            currency: "usd".into(),
            i_participate: true,
            participants: vec![SharedParticipantDraft {
                id: Some("olya".into()),
                name: "Olya".into(),
                ..Default::default()
            }],
            ..SharedEventDraft::default()
        },
    )
    .unwrap()
    .event
    .id;
    settle(&mut left, &mut right);
    crate::usecases::household::shared_events::remove_shared_participant(&mut left, &event, "olya")
        .unwrap();
    pause();
    crate::usecases::household::shared_events::add_shared_purchase(
        &mut right,
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
    let key = crate::usecases::household::categories::create_category(
        &mut left,
        None,
        "Hobby",
        CategoryGroup::Income,
    )
    .unwrap()
    .key;
    settle(&mut left, &mut right);
    crate::usecases::household::categories::delete_category(&mut left, &key).unwrap();
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
    let report = crate::usecases::sync::repair::repair_invariants(&mut left, None).unwrap();
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
    let key = crate::usecases::household::categories::create_category(
        &mut left,
        None,
        "Hobby",
        CategoryGroup::Income,
    )
    .unwrap()
    .key;
    settle(&mut left, &mut right);
    crate::usecases::household::categories::delete_category(&mut left, &key).unwrap();
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
    assert!(
        crate::usecases::sync::repair::repair_invariants(&mut right, None)
            .unwrap()
            .is_empty()
    );
    assert_eq!(right.doc.sync.stamps, stamps);
    assert_eq!(right.doc.sync.tombstones, tombstones);
    assert_eq!(right.doc.notification_log.len(), log);
}

#[test]
fn the_restore_is_told_to_the_user() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let key = crate::usecases::household::categories::create_category(
        &mut left,
        None,
        "Hobby",
        CategoryGroup::Income,
    )
    .unwrap()
    .key;
    settle(&mut left, &mut right);
    crate::usecases::household::categories::delete_category(&mut left, &key).unwrap();
    pause();
    pay_with_category(&mut right, &wallet, &key);
    settle(&mut left, &mut right);
    let notice = right
        .doc
        .notification_log
        .iter()
        // A pass that could not restore yet may have told that first (`unrestored`).
        .find(|record| record.key.starts_with("repair:restored:") && record.body.contains("Hobby"))
        .expect("a notice");
    assert!(notice.body.contains("Hobby"), "{}", notice.body);
    assert!(notice.body.contains("operations still use them") || notice.body.contains("ссылаются"));
}

/// A third device sharing the vault of `left` as it is now.
fn third_device(dir: &tempfile::TempDir, left: &Session) -> Session {
    let copy = dir.path().join("third.wallet");
    std::fs::copy(&left.path, &copy).unwrap();
    let mut third = Session::open(copy, "pw").unwrap();
    peer_sync::set_device(&mut third, "cccccccc-third");
    third
}

fn rename(vault: &mut Session, key: &str, name: &str) {
    crate::usecases::household::categories::update_category(
        vault,
        key,
        Some(name),
        CategoryGroup::Income,
        None,
        None,
    )
    .unwrap();
}

#[test]
fn a_rename_then_a_local_delete_never_brings_the_old_name_back() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    let key = crate::usecases::household::categories::create_category(
        &mut left,
        None,
        "Hobby",
        CategoryGroup::Income,
    )
    .unwrap()
    .key;
    settle(&mut left, &mut right);
    pause();
    rename(&mut right, &key, "Hobby 2");
    settle(&mut left, &mut right);
    // The left took the rename in a batch, then deletes the category itself.
    crate::usecases::household::categories::delete_category(&mut left, &key).unwrap();
    pause();
    pay_with_category(&mut right, &wallet, &key);
    // The deleting device has no batch for its own deletion: nothing to restore.
    pull_one_way(&mut right, &mut left);
    assert!(
        categories_of(&left, &key).is_empty(),
        "no stale value invented"
    );
    let report = crate::usecases::sync::repair::repair_invariants(&mut left, None).unwrap();
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
    let key = crate::usecases::household::categories::create_category(
        &mut left,
        None,
        "Hobby",
        CategoryGroup::Income,
    )
    .unwrap()
    .key;
    settle(&mut left, &mut right);
    let mut third = third_device(&dir, &left);
    pause();
    rename(&mut right, &key, "Hobby 2");
    settle(&mut left, &mut right);
    crate::usecases::household::categories::delete_category(&mut right, &key).unwrap();
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
    assert!(peer_sync::sync_undo(&mut left, &batch).unwrap().restored > 0);
    assert!(
        categories_of(&left, &key).is_empty(),
        "the restore is undone"
    );
}

#[test]
fn a_report_with_only_unrestored_ids_tells_the_user_and_has_its_own_key() {
    let report = super::repair_report::RepairReport {
        unrestored: vec!["transactions:one".into()],
        ..Default::default()
    };
    let english = report.notice_body(false).expect("a notice for unrestored");
    assert!(english.contains("are not on this device"), "{english}");
    assert_eq!(report.notice_title(false), "Waiting for another device");
    assert_eq!(report.notice_title(true), "Ждём другое устройство");
    assert!(
        english.starts_with("Waiting for another device: "),
        "{english}"
    );
    let russian = report.notice_body(true).expect("a notice for unrestored");
    assert!(russian.contains("нет на этом устройстве"), "{russian}");
    let other = super::repair_report::RepairReport {
        unrestored: vec!["transactions:two".into()],
        ..Default::default()
    };
    assert_ne!(report.notice_key(), other.notice_key());
    let mixed = super::repair_report::RepairReport {
        restored: vec![("categories:a".into(), "Hobby".into())],
        unrestored: vec!["transactions:one".into()],
        ..Default::default()
    };
    assert_eq!(mixed.notice_title(false), "Fixed after sync");
    assert_eq!(mixed.notice_title(true), "Исправлено после синхронизации");
}
