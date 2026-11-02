//! A vault file received by transfer, device list decisions and the device
//! tag of stamps written while opening.

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::service::test_support::income;
use crate::sync::state::{CutBy, TransferRecord};

fn cash_id(vault: &UnlockedVault) -> String {
    vault.doc.stores[0].id.clone()
}

#[test]
fn a_received_copy_drops_the_senders_bookkeeping() {
    let (dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    right.add_transaction(income(&cash, "5", "theirs")).unwrap();
    settle(&mut left, &mut right);
    // Left now holds a batch, a journal entry and a stamped record; add the rest.
    let id = left.doc.transactions[0].meta().id.clone();
    left.delete_transaction(&id).unwrap();
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    left.sync_pause_peer("dev-c").unwrap();
    left.doc.sync.peers.last_mut().unwrap().last_synced_at = Some("2026-11-01T10:00:00Z".into());
    left.doc.sync.cut_by.insert(
        "dev-d".into(),
        CutBy {
            reason: crate::ipc_contract::SyncCutReason::Paused,
            at: "2026-11-01T10:00:00Z".into(),
        },
    );
    left.doc.sync.transfers.push(TransferRecord {
        id: "t1".into(),
        ..TransferRecord::default()
    });
    left.doc.sync.tombstones_seen.insert("k".into(), 1);
    left.persist().unwrap();
    let before = left.doc.sync.clone();
    assert!(!before.batches.is_empty() && !before.journal.is_empty());
    assert!(!before.tombstones.is_empty());

    let received = dir.path().join("received.wallet");
    std::fs::copy(&left.path, &received).unwrap();
    let mut mine = UnlockedVault::open(received.clone(), "pw").unwrap();
    mine.adopt_received_copy().unwrap();

    let sync = &mine.doc.sync;
    assert!(sync.paused.is_empty());
    assert!(sync.cut_by.is_empty());
    assert!(sync.batches.is_empty());
    assert!(sync.journal.is_empty());
    assert!(sync.transfers.is_empty());
    assert_eq!(
        sync.tombstones_seen.keys().collect::<Vec<_>>(),
        before.tombstones.keys().collect::<Vec<_>>()
    );
    assert!(!sync.tombstones_seen.contains_key("k"));
    assert!(sync.peers.iter().all(|peer| peer.last_synced_at.is_none()));
    // The record metadata and the trusted devices travel with the data.
    assert_eq!(sync.stamps, before.stamps);
    assert_eq!(sync.tombstones, before.tombstones);
    assert_eq!(sync.peers.len(), before.peers.len());
    drop(mine);
    let reopened = UnlockedVault::open(received, "pw").unwrap();
    assert!(reopened.doc.sync.batches.is_empty());
}

#[test]
fn trusting_again_lifts_a_pause() {
    let (_dir, mut left, _right) = two_devices();
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    left.sync_pause_peer("dev-c").unwrap();
    assert!(left.sync_is_paused("dev-c"));
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    assert!(!left.sync_is_paused("dev-c"));
}

#[test]
fn a_newer_trust_beats_an_older_revoke() {
    let (_dir, mut left, mut right) = two_devices();
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    settle(&mut left, &mut right);
    right
        .sync_merge_peers(&left.sync_state().peers.clone())
        .unwrap();
    left.sync_forget_peer("dev-c").unwrap();
    right
        .sync_merge_peers(&left.sync_state().peers.clone())
        .unwrap();
    assert!(right.sync_state().is_revoked("cc"));
    // The user pairs it again on the right device: a newer stamp.
    right.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    left.sync_merge_peers(&right.sync_state().peers.clone())
        .unwrap();
    right
        .sync_merge_peers(&left.sync_state().peers.clone())
        .unwrap();
    assert!(!left.sync_state().is_revoked("cc"));
    assert!(!right.sync_state().is_revoked("cc"));
    assert!(left.sync_state().trusted("cc").is_some());
    let _ = exchange;
}

#[test]
fn an_older_trust_does_not_lift_a_newer_revoke() {
    let (_dir, mut left, mut right) = two_devices();
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    let trusted = left.sync_state().peers.clone();
    right.sync_merge_peers(&trusted).unwrap();
    left.sync_forget_peer("dev-c").unwrap();
    // The right device still offers the older, live entry.
    left.sync_merge_peers(&right.sync_state().peers.clone())
        .unwrap();
    assert!(left.sync_state().is_revoked("cc"));
    right
        .sync_merge_peers(&left.sync_state().peers.clone())
        .unwrap();
    assert!(right.sync_state().is_revoked("cc"));
}

#[test]
fn open_stamps_carry_this_device() {
    let (dir, mut vault) = crate::service::test_support::empty_vault();
    // An old vault: a built-in category is missing and its batch not yet seen.
    vault.doc.categories.pop().unwrap();
    vault.doc.settings.categories_rev = 0;
    vault.persist().unwrap();
    let path = vault.path.clone();
    let before = vault.doc.sync.stamps.clone();
    drop(vault);

    let opened = UnlockedVault::open_as(path, "pw", Some("cccccccc-this")).unwrap();
    let tag = crate::sync::stamp::device_tag("cccccccc-this");
    let fresh: Vec<_> = opened
        .doc
        .sync
        .stamps
        .iter()
        .filter(|(key, stamps)| before.get(*key) != Some(*stamps))
        .collect();
    assert!(!fresh.is_empty(), "opening stamped nothing");
    for (key, stamps) in fresh {
        assert_eq!(stamps.latest().device, tag, "{key}");
    }
    drop(dir);
}
