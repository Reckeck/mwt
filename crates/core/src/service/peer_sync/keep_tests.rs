//! The «keep» mark: a copy brought back by undoing a merge survives the next
//! batch, on every device that has this build; «Allow merging» lets the
//! dedupe fold it again; an old vault without the field opens.

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::ipc_contract::MergeTarget;
use crate::model::planning::{Reminder, ReminderSource, ReminderSourceKind};
use crate::model::transaction::Transaction;
use crate::service::test_support::income;

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}

/// An automation's operation for one planned moment; its id.
fn rent(vault: &mut UnlockedVault, comment: &str) -> String {
    let cash = vault.doc.stores[0].id.clone();
    let mut tx = income(&cash, "100", comment);
    if let Transaction::Topup(topup) = &mut tx {
        topup.meta.origin = TxOrigin::Automation {
            automation_id: "rent".into(),
            planned_at: "2026-11-01".into(),
        };
    }
    vault.add_transaction(tx).unwrap().meta().id.clone()
}

fn comments(vault: &UnlockedVault) -> Vec<String> {
    let mut out: Vec<String> = vault
        .doc
        .transactions
        .iter()
        .map(|tx| tx.meta().comment.clone())
        .collect();
    out.sort();
    out
}

/// Another batch arrives on both devices, so the dedupe runs on each.
fn touch_and_settle(left: &mut UnlockedVault, right: &mut UnlockedVault, name: &str) {
    left.doc.stores[0].name = name.into();
    left.persist().unwrap();
    right.doc.stores[0].color = "#123456".into();
    right.persist().unwrap();
    settle(left, right);
}

/// Two copies of one moment on `left`; a batch that merged one away; undone.
/// Returns the id of the copy the undo brought back.
fn undone_merge(left: &mut UnlockedVault, right: &mut UnlockedVault) -> String {
    rent(left, "first");
    pause();
    let late = rent(left, "second");
    right.doc.stores[0].color = "#123456".into();
    right.persist().unwrap();
    let (applied_left, _) = exchange(left, right);
    let batch = applied_left.expect("left applied the store change");
    assert_eq!(comments(left), vec!["first"], "the later copy was merged");
    let report = left.sync_undo(&batch.id).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(comments(left), vec!["first", "second"]);
    late
}

fn kept(vault: &UnlockedVault, id: &str) -> bool {
    vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == id)
        .is_some_and(|tx| tx.meta().keep)
}

#[test]
fn the_copy_survives_the_next_batch_after_undo() {
    let (_dir, mut left, mut right) = two_devices();
    let late = undone_merge(&mut left, &mut right);
    assert!(kept(&left, &late), "undo did not mark the copy");
    touch_and_settle(&mut left, &mut right, "Renamed");
    assert_eq!(comments(&left), vec!["first", "second"]);
}

#[test]
fn the_other_device_honours_the_mark_too() {
    let (_dir, mut left, mut right) = two_devices();
    let late = undone_merge(&mut left, &mut right);
    touch_and_settle(&mut left, &mut right, "Renamed");
    assert_eq!(comments(&right), vec!["first", "second"]);
    assert!(
        kept(&right, &late),
        "the mark did not reach the other device"
    );
    // And once more, on a later batch from the other side.
    right.doc.stores[0].name = "Again".into();
    right.persist().unwrap();
    settle(&mut left, &mut right);
    assert_eq!(comments(&left), comments(&right));
    assert_eq!(comments(&right), vec!["first", "second"]);
}

#[test]
fn a_plain_edit_does_not_clear_the_mark() {
    let (_dir, mut left, mut right) = two_devices();
    let late = undone_merge(&mut left, &mut right);
    let mut edited = left
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == late)
        .unwrap()
        .clone();
    edited.meta_mut().comment = "second, edited".into();
    edited.meta_mut().keep = false; // a form does not know the mark
    left.update_transaction(&late, edited).unwrap();
    assert!(kept(&left, &late), "the edit cleared the mark");
    touch_and_settle(&mut left, &mut right, "Renamed");
    assert_eq!(comments(&left), vec!["first", "second, edited"]);
}

#[test]
fn allow_merging_clears_the_mark_and_the_next_dedupe_folds() {
    let (_dir, mut left, mut right) = two_devices();
    let late = undone_merge(&mut left, &mut right);
    left.allow_merging(MergeTarget::Transaction, &late).unwrap();
    assert!(!kept(&left, &late));
    touch_and_settle(&mut left, &mut right, "Renamed");
    assert_eq!(comments(&left), vec!["first"]);
    assert_eq!(comments(&right), comments(&left));
    assert!(matches!(
        left.allow_merging(MergeTarget::Transaction, "no-such-id"),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn a_kept_reminder_survives_too() {
    let (_dir, mut left, mut right) = two_devices();
    let source = ReminderSource {
        kind: ReminderSourceKind::LoanPayment,
        id: "loan".into(),
    };
    let make = |id: &str| {
        let mut reminder = Reminder::new("Pay", "2026-03-25T09:00:00Z");
        reminder.id = id.into();
        reminder.source = Some(source.clone());
        reminder
    };
    left.doc.reminders.push(make("a-first"));
    left.persist().unwrap();
    pause();
    left.doc.reminders.push(make("b-second"));
    left.persist().unwrap();
    right.doc.stores[0].color = "#123456".into();
    right.persist().unwrap();
    let (applied_left, _) = exchange(&mut left, &mut right);
    let batch = applied_left.expect("left applied the store change");
    assert_eq!(left.doc.reminders.len(), 1, "the copy was merged");
    left.sync_undo(&batch.id).unwrap();
    assert_eq!(left.doc.reminders.len(), 2);
    assert!(left.doc.reminders.iter().any(|reminder| reminder.keep));
    touch_and_settle(&mut left, &mut right, "Renamed");
    assert_eq!(left.doc.reminders.len(), 2);
    assert_eq!(right.doc.reminders.len(), 2);
    let marked = left
        .doc
        .reminders
        .iter()
        .find(|reminder| reminder.keep)
        .unwrap()
        .id
        .clone();
    left.allow_merging(MergeTarget::Reminder, &marked).unwrap();
    touch_and_settle(&mut left, &mut right, "Renamed again");
    assert_eq!(left.doc.reminders.len(), 1);
    assert_eq!(right.doc.reminders.len(), 1);
}

#[test]
fn an_old_vault_without_the_field_opens() {
    let (_dir, mut left, _right) = two_devices();
    rent(&mut left, "old");
    let mut value = serde_json::to_value(&left.doc).unwrap();
    // An old file has no `keep` at all: strip it from what is written now.
    for tx in value["transactions"].as_array_mut().unwrap() {
        for body in tx.as_object_mut().unwrap().values_mut() {
            if let Some(meta) = body.get_mut("meta").and_then(|meta| meta.as_object_mut()) {
                meta.remove("keep");
            }
        }
    }
    value["reminders"] = serde_json::json!([{
        "id": "r", "text": "Pay", "due_at": "2026-03-25T09:00:00Z", "done": false
    }]);
    let doc: crate::model::vault::VaultDoc = serde_json::from_value(value).unwrap();
    assert!(doc.transactions.iter().all(|tx| !tx.meta().keep));
    assert!(!doc.reminders[0].keep);
    // MessagePack as the release build writes it (named fields).
    let bytes = rmp_serde::to_vec_named(&doc).unwrap();
    let back: crate::model::vault::VaultDoc = rmp_serde::from_slice(&bytes).unwrap();
    assert!(!back.reminders[0].keep);
    // A marked record round-trips in both.
    let mut marked = doc.clone();
    marked.reminders[0].keep = true;
    let json = serde_json::to_string(&marked).unwrap();
    let back: crate::model::vault::VaultDoc = serde_json::from_str(&json).unwrap();
    assert!(back.reminders[0].keep);
    let bytes = rmp_serde::to_vec_named(&marked).unwrap();
    let back: crate::model::vault::VaultDoc = rmp_serde::from_slice(&bytes).unwrap();
    assert!(back.reminders[0].keep);
}
