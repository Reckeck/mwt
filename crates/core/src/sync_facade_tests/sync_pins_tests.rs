//! Characterization pins of the vault side of sync (spec core-strangler, D6 step 2): what the
//! manifest, diff, records, preview, history, batch and trust methods answer, written before they
//! moved so the move can be checked against them.

use std::collections::BTreeMap;

use crate::crypto::level::SecurityLevel;
use crate::service::UnlockedVault;
use crate::service::test_support::{empty_vault, income};
use crate::sync::plan::ChangeKind;
use crate::sync::{RecordState, SyncSummary};

/// The same vault on two devices: a copy of one file, opened twice.
fn two_devices() -> (tempfile::TempDir, UnlockedVault, UnlockedVault) {
    let (dir, mut left) = empty_vault();
    left.set_device("aaaaaaaa-left");
    left.persist().unwrap();
    let copy = dir.path().join("copy.wallet");
    std::fs::copy(&left.path, &copy).unwrap();
    let mut right = UnlockedVault::open(copy, "pw").unwrap();
    right.set_device("bbbbbbbb-right");
    (dir, left, right)
}

fn cash_id(vault: &UnlockedVault) -> String {
    vault.doc.stores[0].id.clone()
}

/// What `from` holds that `into` lacks or has older, as a session sends it.
fn batch_for(into: &mut UnlockedVault, from: &mut UnlockedVault) -> BTreeMap<String, RecordState> {
    let mine = into.sync_manifest(None).unwrap();
    let theirs = from.sync_manifest(None).unwrap();
    let (wanted, _) = into.sync_diff(&mine, &theirs);
    from.sync_records(&wanted).unwrap()
}

fn peer() -> crate::service::PeerRef {
    crate::service::PeerRef {
        device_id: "right-id".into(),
        name: "right".into(),
    }
}

#[test]
fn sync_manifest_digests_records_and_is_stable_between_calls() {
    let (_dir, mut left, _right) = two_devices();
    let cash = cash_id(&left);
    let first = left.sync_manifest(None).unwrap();
    assert!(first.contains_key(&format!("stores:{cash}")));
    assert_eq!(left.sync_manifest(None).unwrap(), first);
}

#[test]
fn sync_manifest_lists_a_deletion_by_its_key() {
    let (_dir, mut left, _right) = two_devices();
    let cash = cash_id(&left);
    let tx = left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let key = format!("transactions:{}", tx.meta().id);
    assert!(left.sync_manifest(None).unwrap().contains_key(&key));
    left.delete_transaction(&tx.meta().id).unwrap();
    assert!(left.sync_manifest(None).unwrap().contains_key(&key));
    assert!(left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn sync_manifest_for_a_full_peer_withholds_nothing() {
    let (_dir, mut left, _right) = two_devices();
    let plain = left.sync_manifest(None).unwrap();
    let (manifest, withheld) = left.sync_manifest_for(None, SecurityLevel::Crypto).unwrap();
    assert_eq!(manifest, plain);
    assert_eq!(withheld, 0);
}

#[test]
fn sync_manifest_for_a_lower_peer_counts_only_crypto_records() {
    let (_dir, mut left, _right) = two_devices();
    let plain = left.sync_manifest(None).unwrap();
    let (manifest, withheld) = left.sync_manifest_for(None, SecurityLevel::Pin).unwrap();
    assert_eq!(manifest, plain);
    assert_eq!(withheld, 0);
}

#[test]
fn sync_manifest_marked_carries_the_clock_of_the_moment() {
    let (_dir, mut left, _right) = two_devices();
    let (manifest, mark) = left.sync_manifest_marked(None).unwrap();
    let (again, same_mark) = left.sync_manifest_marked(None).unwrap();
    assert_eq!(manifest, left.sync_manifest(None).unwrap());
    assert_eq!((again, same_mark), (manifest, mark));
}

#[test]
fn sync_manifest_marked_for_adds_the_withheld_count() {
    let (_dir, mut left, _right) = two_devices();
    let (manifest, mark, withheld) = left
        .sync_manifest_marked_for(None, SecurityLevel::Crypto)
        .unwrap();
    assert_eq!(manifest, left.sync_manifest(None).unwrap());
    assert_eq!(mark, left.sync_manifest_marked(None).unwrap().1);
    assert_eq!(withheld, 0);
}

#[test]
fn sync_diff_asks_for_what_differs_and_offers_what_the_other_lacks() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    let tx = left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let key = format!("transactions:{}", tx.meta().id);
    let mine = left.sync_manifest(None).unwrap();
    let theirs = right.sync_manifest(None).unwrap();
    let (request, send) = right.sync_diff(&theirs, &mine);
    assert!(request.contains(&key));
    assert!(send.is_empty() || !send.contains(&key));
    let (request, send) = left.sync_diff(&mine, &theirs);
    assert!(!request.contains(&key));
    assert!(send.contains(&key));
}

#[test]
fn sync_diff_skips_a_rate_point_this_device_would_thin_away() {
    let (_dir, left, _right) = two_devices();
    let mine = BTreeMap::new();
    let mut theirs = BTreeMap::new();
    theirs.insert("fx_cache:zzz|2020-01-01".to_string(), 1u64);
    theirs.insert("stores:other".to_string(), 2u64);
    let (request, _) = left.sync_diff(&mine, &theirs);
    assert_eq!(request, vec!["stores:other".to_string()]);
}

#[test]
fn sync_records_answers_asked_keys_and_leaves_malformed_ones_out() {
    let (_dir, mut left, _right) = two_devices();
    let cash = cash_id(&left);
    let key = format!("stores:{cash}");
    let records = left
        .sync_records(&[key.clone(), "no-colon".to_string()])
        .unwrap();
    assert_eq!(records.len(), 1);
    let state = &records[&key];
    assert!(state.value.is_some());
    assert!(state.tombstone.is_none());
}

#[test]
fn sync_records_gives_a_deleted_record_as_its_tombstone() {
    let (_dir, mut left, _right) = two_devices();
    let cash = cash_id(&left);
    let tx = left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let key = format!("transactions:{}", tx.meta().id);
    left.delete_transaction(&tx.meta().id).unwrap();
    let records = left.sync_records(std::slice::from_ref(&key)).unwrap();
    assert!(records[&key].value.is_none());
    assert!(records[&key].tombstone.is_some());
}

#[test]
fn sync_records_for_answers_a_full_peer_as_sync_records_does() {
    let (_dir, mut left, _right) = two_devices();
    let key = format!("stores:{}", cash_id(&left));
    let keys = [key];
    assert_eq!(
        left.sync_records_for(&keys, SecurityLevel::Crypto).unwrap(),
        left.sync_records(&keys).unwrap()
    );
}

#[test]
fn sync_preview_lists_what_a_batch_would_add_and_changes_nothing() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let incoming = batch_for(&mut right, &mut left);
    let before = right.doc.transactions.len();
    let summary = right.sync_preview(&incoming).unwrap();
    let group = summary
        .collections
        .iter()
        .find(|group| group.collection == "transactions")
        .expect("transactions listed");
    assert_eq!(group.items[0].kind, ChangeKind::Added);
    assert_eq!(right.doc.transactions.len(), before);
}

#[test]
fn sync_preview_from_matches_the_plain_preview_for_an_unknown_device() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let incoming = batch_for(&mut right, &mut left);
    assert_eq!(
        right.sync_preview_from(&incoming, "nobody").unwrap(),
        right.sync_preview(&incoming).unwrap()
    );
}

#[test]
fn sync_preview_refuses_a_record_filed_under_another_key() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    let tx = left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let mut incoming = batch_for(&mut right, &mut left);
    let key = format!("transactions:{}", tx.meta().id);
    let state = incoming.remove(&key).unwrap();
    incoming.insert("transactions:another-id".to_string(), state);
    assert!(right.sync_preview(&incoming).is_err());
    assert!(right.sync_preview_after(&incoming).is_err());
}

#[test]
fn sync_preview_after_reports_the_balance_a_batch_would_leave() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let incoming = batch_for(&mut right, &mut left);
    let after = right
        .sync_preview_after(&incoming)
        .unwrap()
        .expect("the batch changes something");
    assert_eq!(after.stores.len(), 1);
    assert_eq!(after.stores[0].id, cash);
    assert!(after.net_worth_after > after.net_worth_before);
    assert!(right.doc.transactions.is_empty());
}

#[test]
fn sync_preview_after_is_none_for_an_empty_batch() {
    let (_dir, _left, mut right) = two_devices();
    assert!(
        right
            .sync_preview_after(&BTreeMap::new())
            .unwrap()
            .is_none()
    );
}

#[test]
fn sync_history_lists_an_applied_batch_and_a_transfer_newest_first() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let incoming = batch_for(&mut right, &mut left);
    let applied = right.sync_apply(&incoming, &peer()).unwrap().unwrap();
    assert_eq!(right.sync_history(), vec![applied.clone()]);
    right
        .sync_record_transfer("dev-c", "Tablet", "cc", false)
        .unwrap();
    let history = right.sync_history();
    assert_eq!(history.len(), 2);
    let transfer = history
        .iter()
        .find(|item| item.transferred_to.is_some())
        .expect("transfer listed");
    assert_eq!(transfer.peer_name, "Tablet");
    assert_eq!(transfer.summary, SyncSummary::default());
    assert!(history.contains(&applied));
}

#[test]
fn sync_batch_keys_and_collections_name_what_a_batch_changed() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    let tx = left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let incoming = batch_for(&mut right, &mut left);
    let applied = right.sync_apply(&incoming, &peer()).unwrap().unwrap();
    let keys = right.sync_batch_keys(&applied.id);
    assert!(keys.contains(&format!("transactions:{}", tx.meta().id)));
    let collections = right.sync_batch_collections(&applied.id);
    assert!(collections.contains(&"transactions".to_string()));
    let mut sorted = collections.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(collections, sorted);
}

#[test]
fn sync_batch_methods_answer_empty_for_an_unknown_batch() {
    let (_dir, left, _right) = two_devices();
    assert!(left.sync_batch_keys("nope").is_empty());
    assert!(left.sync_batch_collections("nope").is_empty());
    assert!(left.sync_batch_notices("nope").is_empty());
}

#[test]
fn sync_batch_notices_hands_back_the_notices_a_batch_added() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    left.add_transaction(income(&cash, "10", "salary")).unwrap();
    let incoming = batch_for(&mut right, &mut left);
    let applied = right.sync_apply(&incoming, &peer()).unwrap().unwrap();
    let added: Vec<String> = right
        .sync_batch_notices(&applied.id)
        .iter()
        .map(|record| record.key.clone())
        .collect();
    let from_keys: Vec<String> = right
        .sync_batch_keys(&applied.id)
        .iter()
        .filter_map(|key| key.strip_prefix("notification_log:"))
        .map(str::to_string)
        .collect();
    assert_eq!(added.len(), from_keys.len());
}

#[test]
fn sync_trust_peer_adds_a_device_and_pairs_a_known_one_afresh() {
    let (_dir, mut left, _right) = two_devices();
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    let state = left.sync_state();
    assert_eq!(state.peers.len(), 1);
    let peer = &state.peers[0];
    assert_eq!(peer.device_id, "dev-c");
    assert_eq!(peer.name, "Tablet");
    assert_eq!(peer.public_key, "cc");
    assert!(peer.revoked.is_none());
    assert!(peer.last_synced_at.is_none());
    let first_stamp = peer.stamp.clone();
    left.sync_pause_peer("dev-c").unwrap();
    left.sync_trust_peer("dev-c", "Tablet 2", "cc").unwrap();
    let state = left.sync_state();
    assert_eq!(state.peers.len(), 1);
    assert_eq!(state.peers[0].name, "Tablet 2");
    assert!(state.peers[0].stamp > first_stamp);
    assert!(!left.sync_is_paused("dev-c"));
}

#[test]
fn sync_forget_peer_revokes_a_device_and_refuses_an_unknown_one() {
    let (_dir, mut left, _right) = two_devices();
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    left.sync_pause_peer("dev-c").unwrap();
    left.sync_forget_peer("dev-c").unwrap();
    let peer = &left.sync_state().peers[0];
    assert_eq!(peer.revoked.as_ref(), Some(&peer.stamp));
    assert!(!left.sync_is_paused("dev-c"));
    let error = left.sync_forget_peer("ghost").unwrap_err();
    assert!(matches!(error, crate::error::Error::NotFound(_)));
}

/// `sync_forget_peer` makes the clock pass the stamps of every entry held for
/// the device id (not only the first), so the revocation sorts after all of
/// them; pinned through the revocation stamp on a vault holding two entries.
#[test]
fn sync_forget_peer_observes_the_stamps_of_every_entry_with_the_device_id() {
    use crate::sync::stamp::Stamp;
    let peer = |device: &str, wall: u64| crate::sync::TrustedPeer {
        device_id: device.into(),
        name: "Phone".into(),
        public_key: "cc".into(),
        paired_at: "2026-10-01T00:00:00Z".into(),
        last_synced_at: None,
        revoked: None,
        stamp: Stamp::new(wall, 0, "zz"),
    };
    let (_dir, mut left, _right) = two_devices();
    left.doc.sync.peers = vec![peer("dev-c", 10), peer("other", 99), peer("dev-c", 50)];
    left.sync_forget_peer("dev-c").unwrap();
    let revoked = left
        .sync_state()
        .peers
        .iter()
        .filter(|entry| entry.device_id == "dev-c")
        .filter_map(|entry| entry.revoked.clone())
        .max()
        .expect("a dev-c entry is revoked");
    assert!(revoked > Stamp::new(50, 0, "zz"));
}
