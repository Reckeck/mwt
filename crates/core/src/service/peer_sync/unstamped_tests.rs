//! Two copies of a vault that diverged before field stamps existed: the merge
//! cannot know who changed what. It picks the same value as ever, but says so.

use super::tests::{exchange, two_devices};
use super::*;
use crate::model::transaction::Transaction;
use crate::service::test_support::income;

const COPY: &str = "Copy";

fn comment_of(vault: &UnlockedVault, id: &str) -> String {
    vault
        .doc
        .transactions
        .iter()
        .find_map(|tx| match tx {
            Transaction::Topup(topup) if topup.meta.id == id => Some(topup.meta.comment.clone()),
            _ => None,
        })
        .expect("the transaction exists")
}

fn set_comment(vault: &mut UnlockedVault, id: &str, text: &str) {
    for tx in &mut vault.doc.transactions {
        if let Transaction::Topup(topup) = tx
            && topup.meta.id == id
        {
            topup.meta.comment = text.into();
        }
    }
    vault.persist().unwrap();
}

/// Both devices hold one transaction, with `left_text` here and `right_text`
/// there, and neither has a stamp for anything.
fn unstamped_pair(
    left_text: &str,
    right_text: &str,
) -> (tempfile::TempDir, UnlockedVault, UnlockedVault, String) {
    let (dir, mut left, mut right) = two_devices();
    let cash = left.doc.stores[0].id.clone();
    left.add_transaction(income(&cash, "10", "base")).unwrap();
    exchange(&mut left, &mut right);
    let id = left.doc.transactions[0].meta().id.clone();
    set_comment(&mut left, &id, left_text);
    set_comment(&mut right, &id, right_text);
    left.doc.sync.stamps.clear();
    right.doc.sync.stamps.clear();
    (dir, left, right, id)
}

#[test]
fn different_unstamped_values_are_counted_and_the_loser_is_listed() {
    let (_dir, mut left, mut right, id) = unstamped_pair("aaa", "zzz");
    let applied = left
        .merge_from(&mut right, COPY)
        .unwrap()
        .expect("the copy changes this vault");
    // The pick is the one the value comparison always made: the larger one.
    assert_eq!(comment_of(&left, &id), "zzz");
    assert_eq!(applied.summary.unstamped_conflicts, 1);
    assert_eq!(applied.summary.overwritten.len(), 1);
    assert_eq!(applied.summary.overwritten[0].collection, "transactions");
    assert_eq!(applied.summary.overwritten[0].before, "aaa");
    assert_eq!(applied.summary.overwritten[0].after, "zzz");
}

#[test]
fn when_the_local_value_wins_nothing_is_overwritten_or_counted() {
    let (_dir, mut left, mut right, id) = unstamped_pair("zzz", "aaa");
    let applied = left.merge_from(&mut right, COPY).unwrap();
    assert_eq!(comment_of(&left, &id), "zzz");
    assert!(
        applied.is_none_or(|batch| batch.summary.overwritten.is_empty()),
        "the local value was kept: nothing to list"
    );
}

#[test]
fn equal_unstamped_copies_have_no_conflict() {
    let (_dir, mut left, mut right, id) = unstamped_pair("same", "same");
    let applied = left.merge_from(&mut right, COPY).unwrap();
    assert_eq!(comment_of(&left, &id), "same");
    assert!(applied.is_none_or(
        |batch| batch.summary.unstamped_conflicts == 0 && batch.summary.overwritten.is_empty()
    ));
}

#[test]
fn a_peer_over_the_network_neither_counts_nor_lists_unstamped_values() {
    // The same pair of values reaches this device from a paired device
    // (`sync_apply`, not the external copy path): the value picked is the
    // same, and the summary stays as it was.
    let (_dir, mut left, mut right, id) = unstamped_pair("aaa", "zzz");
    let wanted = vec![format!("transactions:{id}")];
    let incoming = right.sync_records(&wanted).unwrap();
    let peer = PeerRef {
        device_id: "right-id".into(),
        name: "right".into(),
    };
    let applied = left
        .sync_apply(&incoming, &peer)
        .unwrap()
        .expect("the value changes");
    assert_eq!(comment_of(&left, &id), "zzz");
    assert_eq!(applied.summary.unstamped_conflicts, 0);
    assert!(applied.summary.overwritten.is_empty());
}
