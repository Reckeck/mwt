//! Every deletion from a log leaves a tombstone (undo, a deleted budget), and
//! a capped log keeps the same records on every device.

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::Amount;
use crate::model::notify::{MAX_NOTICE_LOG, NoticeCategory, NoticeRecord};
use crate::model::planning::{BudgetPeriod, BudgetTarget};
use crate::service::budget::BudgetDraft;

fn notice(key: &str, delivered_at: &str) -> NoticeRecord {
    NoticeRecord {
        key: key.into(),
        category: NoticeCategory::Budget,
        title: key.into(),
        body: String::new(),
        link: None,
        at: delivered_at.into(),
        delivered_at: delivered_at.into(),
        read: false,
    }
}

fn keys(vault: &UnlockedVault) -> Vec<String> {
    let mut keys: Vec<String> = vault
        .doc
        .notification_log
        .iter()
        .map(|record| record.key.clone())
        .collect();
    keys.sort();
    keys
}

fn peer(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

/// Pull right's records into left and return the id of the batch it made.
fn exchange_batch(left: &mut UnlockedVault, right: &mut UnlockedVault) -> String {
    let (applied_left, _) = exchange(left, right);
    applied_left.expect("left applied a batch").id
}

#[test]
fn undo_of_an_added_log_record_does_not_come_back() {
    let (_dir, mut left, mut right) = two_devices();
    right
        .doc
        .notification_log
        .push(notice("reminder:a", "2026-11-01T10:00:00Z"));
    let batch = exchange_batch(&mut left, &mut right);
    assert_eq!(keys(&left), vec!["reminder:a"]);

    let report = left.sync_undo(&batch).unwrap();
    assert_eq!(report.restored, 1);
    assert!(keys(&left).is_empty());
    assert!(
        left.doc
            .sync
            .tombstones
            .contains_key("notification_log:reminder:a")
    );

    // The record does not return from the peer, and the deletion reaches it.
    settle(&mut left, &mut right);
    assert!(keys(&left).is_empty(), "{:?}", keys(&left));
    assert!(keys(&right).is_empty(), "{:?}", keys(&right));
}

#[test]
fn undo_lists_a_union_entry_as_skipped() {
    let (_dir, mut left, mut right) = two_devices();
    right.doc.discarded_imports.push("exec-1".into());
    let batch = exchange_batch(&mut left, &mut right);
    assert_eq!(left.doc.discarded_imports, vec!["exec-1".to_string()]);

    let report = left.sync_undo(&batch).unwrap();
    assert_eq!(report.restored, 0, "a union entry cannot be deleted");
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
    assert_eq!(left.doc.discarded_imports, vec!["exec-1".to_string()]);
}

#[test]
fn a_deleted_budget_does_not_bring_its_notices_back() {
    let (_dir, mut left, mut right) = two_devices();
    let budget = left
        .create_budget(BudgetDraft {
            active: true,
            target: BudgetTarget::Category("groceries".into()),
            period: BudgetPeriod::Month,
            limit: Amount::parse("100").unwrap(),
            currency: "rub".into(),
            warn: true,
        })
        .unwrap();
    let key = format!("budget:{}:2026-11:over", budget.id);
    left.doc
        .notification_log
        .push(notice(&key, "2026-11-01T10:00:00Z"));
    settle(&mut left, &mut right);
    assert_eq!(keys(&right), vec![key.clone()]);

    left.delete_budget(&budget.id).unwrap();
    assert!(keys(&left).is_empty());
    settle(&mut left, &mut right);
    assert!(keys(&left).is_empty(), "{:?}", keys(&left));
    assert!(keys(&right).is_empty(), "{:?}", keys(&right));
}

fn fill(vault: &mut UnlockedVault, prefix: &str, count: usize) {
    for index in 0..count {
        // Every record shares one delivery time: only the key can tell them apart.
        vault.doc.notification_log.push(notice(
            &format!("{prefix}:{index:04}"),
            "2026-11-01T10:00:00Z",
        ));
    }
}

#[test]
fn two_devices_trim_a_log_to_the_same_records() {
    let (_dir, mut left, mut right) = two_devices();
    fill(&mut left, "a", MAX_NOTICE_LOG);
    fill(&mut right, "b", 5);
    // Opposite array order on the two devices.
    right.doc.notification_log.reverse();
    for _ in 0..3 {
        exchange(&mut left, &mut right);
    }
    assert_eq!(left.doc.notification_log.len(), MAX_NOTICE_LOG);
    assert!(
        keys(&left) == keys(&right),
        "devices kept different records"
    );
}

#[test]
fn a_record_cut_by_the_cap_is_not_counted_or_requested_again() {
    let (_dir, mut left, mut right) = two_devices();
    // Left holds a full log of recent records; right has one older record.
    for index in 0..MAX_NOTICE_LOG {
        left.doc
            .notification_log
            .push(notice(&format!("new:{index:04}"), "2026-11-02T10:00:00Z"));
    }
    right
        .doc
        .notification_log
        .push(notice("old:0000", "2026-10-01T10:00:00Z"));
    let right_manifest = right.sync_manifest(None).unwrap();
    let left_manifest = left.sync_manifest(None).unwrap();
    let (wants, _) = left.sync_diff(&left_manifest, &right_manifest);
    let incoming = right.sync_records(&wants).unwrap();
    let before = left.doc.sync.batches.len();
    let applied = left.apply_batch(&incoming, &peer("right"), false).unwrap();
    assert!(
        applied.is_none(),
        "the cut record made a batch: {applied:?}"
    );
    assert_eq!(left.doc.sync.batches.len(), before);
    assert!(!keys(&left).contains(&"old:0000".to_string()));
}
