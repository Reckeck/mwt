//! "Overwritten" lists only what this device edited and the other device had
//! not seen yet (`SyncState.seen_by`), and a lost change of a record's kind.

use std::time::Duration;

use super::tests::{settle, two_devices};
use super::*;
use crate::model::transaction::Transaction;
use crate::service::test_support::{income, payment};
use crate::sync::plan::OverwrittenField;

fn peer_of(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

/// One exchange as a live session runs it: the clock mark is taken with the
/// manifest, and an applied exchange records it as what the peer has seen.
fn exchange_marked(
    left: &mut UnlockedVault,
    right: &mut UnlockedVault,
) -> (Vec<OverwrittenField>, Vec<OverwrittenField>) {
    let (left_manifest, left_mark) = left.sync_manifest_marked(None).unwrap();
    let (right_manifest, right_mark) = right.sync_manifest_marked(None).unwrap();
    let (left_wants, _) = left.sync_diff(&left_manifest, &right_manifest);
    let (right_wants, _) = right.sync_diff(&right_manifest, &left_manifest);
    let to_left = right.sync_records(&left_wants).unwrap();
    let to_right = left.sync_records(&right_wants).unwrap();
    let applied_left = left.sync_apply(&to_left, &peer_of("right")).unwrap();
    let applied_right = right.sync_apply(&to_right, &peer_of("left")).unwrap();
    left.sync_mark_synced("right-id", Some(&left_mark)).unwrap();
    right
        .sync_mark_synced("left-id", Some(&right_mark))
        .unwrap();
    let overwritten = |applied: Option<AppliedBatch>| {
        applied
            .map(|batch| batch.summary.overwritten)
            .unwrap_or_default()
    };
    (overwritten(applied_left), overwritten(applied_right))
}

/// Let the wall clock move on, so the next stamp of the other device is
/// certainly the later one.
fn later() {
    std::thread::sleep(Duration::from_millis(3));
}

fn comment_edit(vault: &mut UnlockedVault, id: &str, comment: &str) {
    let mut edited = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == id)
        .expect("transaction")
        .clone();
    edited.meta_mut().comment = comment.into();
    vault.update_transaction(id, edited).unwrap();
}

fn seeded() -> (tempfile::TempDir, UnlockedVault, UnlockedVault, String) {
    let (dir, mut left, mut right) = two_devices();
    let cash = left.doc.stores[0].id.clone();
    left.add_transaction(income(&cash, "10", "salary")).unwrap();
    settle(&mut left, &mut right);
    let id = left.doc.transactions[0].meta().id.clone();
    (dir, left, right, id)
}

#[test]
fn an_edit_the_peer_already_saw_is_not_an_overwrite() {
    let (_dir, mut left, mut right, id) = seeded();
    comment_edit(&mut left, &id, "first");
    exchange_marked(&mut left, &mut right);
    later();
    comment_edit(&mut right, &id, "second");
    let (on_left, on_right) = exchange_marked(&mut left, &mut right);
    assert!(on_left.is_empty(), "the peer had seen it: {on_left:?}");
    assert!(on_right.is_empty());
    assert_eq!(left.doc.transactions[0].meta().comment, "second");
}

#[test]
fn an_edit_the_peer_has_not_seen_is_an_overwrite() {
    let (_dir, mut left, mut right, id) = seeded();
    comment_edit(&mut left, &id, "first");
    exchange_marked(&mut left, &mut right);
    comment_edit(&mut left, &id, "mine, unseen");
    later();
    comment_edit(&mut right, &id, "theirs");
    let (on_left, _) = exchange_marked(&mut left, &mut right);
    assert_eq!(on_left.len(), 1, "{on_left:?}");
    assert_eq!(on_left[0].after, "theirs");
}

#[test]
fn without_a_record_of_the_peer_every_edit_still_counts() {
    let (_dir, mut left, mut right, id) = seeded();
    comment_edit(&mut left, &id, "mine");
    later();
    comment_edit(&mut right, &id, "theirs");
    let batch = right.sync_records(&[format!("transactions:{id}")]).unwrap();
    let applied = left.sync_apply(&batch, &peer_of("right")).unwrap().unwrap();
    assert_eq!(applied.summary.overwritten.len(), 1);
}

#[test]
fn seen_by_only_grows_and_is_not_taken_from_a_received_copy() {
    let (_dir, mut left, mut right, id) = seeded();
    comment_edit(&mut left, &id, "first");
    exchange_marked(&mut left, &mut right);
    let seen = left.sync_state().seen_by["right-id"].clone();
    left.sync_mark_synced("right-id", Some(&Stamp::new(1, 0, &seen.device)))
        .unwrap();
    assert_eq!(left.sync_state().seen_by["right-id"], seen);
    left.sync_mark_synced("right-id", None).unwrap();
    assert_eq!(left.sync_state().seen_by["right-id"], seen);

    let mut sent = left.sync_state().clone();
    assert!(!sent.seen_by.is_empty());
    let mut keeper = crate::sync::SyncState::default();
    keeper.seen_by.insert("x-id".into(), seen);
    keeper.take_records_from(std::mem::take(&mut sent), 1);
    assert_eq!(keeper.seen_by.len(), 1, "a pull keeps its own seen_by");
    assert!(keeper.seen_by.contains_key("x-id"));
}

#[test]
fn a_lost_change_of_kind_is_listed_as_overwritten() {
    let (_dir, mut left, mut right, id) = seeded();
    let cash = left.doc.stores[0].id.clone();
    let mut as_payment = payment(&cash, "10", "salary");
    as_payment.meta_mut().id = id.clone();
    left.update_transaction(&id, as_payment).unwrap();
    assert!(matches!(left.doc.transactions[0], Transaction::Payment(_)));
    later();
    comment_edit(&mut right, &id, "edited as a top-up");
    let (on_left, on_right) = exchange_marked(&mut left, &mut right);
    assert_eq!(on_left.len(), 1, "{on_left:?}");
    assert_eq!(on_left[0].field, "kind");
    assert_eq!(on_left[0].before, "payment");
    assert_eq!(on_left[0].after, "topup");
    assert!(on_right.is_empty());
}
