//! Deletions live until every device that counts has them (acks from the
//! peers' manifests), a forgotten device counts for a year, and a deletion
//! that was already compacted elsewhere is not news.

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::model::contact::ContactKind;
use crate::service::contacts::ContactDraft;
use crate::sync::RecordStamps;

const DAY_MS: u64 = 24 * 3600 * 1000;

fn days_ago(days: u64) -> u64 {
    crate::sync::stamp::now_ms() - days * DAY_MS
}

fn peer(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

/// A contact both devices hold with stamps from long ago, then deleted on
/// `left`, the deletion aged `age_days`. Returns the record key.
fn deleted_long_ago(left: &mut UnlockedVault, right: &mut UnlockedVault, age_days: u64) -> String {
    let contact = left
        .create_contact(ContactDraft {
            name: "Gone".into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        })
        .unwrap();
    settle(left, right);
    let key = format!("contacts:{}", contact.id);
    let created = Stamp::new(days_ago(age_days + 100), 0, "aaaaaaaa");
    for vault in [&mut *left, &mut *right] {
        vault
            .doc
            .sync
            .stamps
            .insert(key.clone(), RecordStamps::created(created.clone()));
        vault.persist().unwrap();
    }
    left.delete_contact(&contact.id).unwrap();
    age_tombstone(left, &key, age_days);
    key
}

fn age_tombstone(vault: &mut UnlockedVault, key: &str, age_days: u64) {
    let old = days_ago(age_days);
    vault
        .doc
        .sync
        .tombstones
        .insert(key.to_string(), Stamp::new(old, 0, "aaaaaaaa"));
    vault.doc.sync.tombstones_seen.insert(key.to_string(), old);
}

fn age_revocation(vault: &mut UnlockedVault, device_id: &str, age_days: u64) {
    let stamp = Stamp::new(days_ago(age_days), 0, "aaaaaaaa");
    let entry = vault
        .doc
        .sync
        .peers
        .iter_mut()
        .find(|peer| peer.device_id == device_id)
        .unwrap();
    entry.revoked = Some(stamp.clone());
    entry.stamp = stamp;
}

#[test]
fn a_declined_session_does_not_ack_tombstones() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 200);
    left.sync_trust_peer("right-id", "Right", "rr").unwrap();
    // The peer has the deletion; the session only compared manifests and
    // was declined: nothing is recorded, the deletion stays.
    right
        .sync_apply(
            &left.sync_records(std::slice::from_ref(&key)).unwrap(),
            &peer("left"),
        )
        .unwrap();
    let theirs = right.sync_manifest(None).unwrap();
    let mine = left.sync_manifest(None).unwrap();
    let _ = left.sync_diff(&mine, &theirs);
    left.sync_mark_synced("right-id", None).unwrap();
    left.compact_tombstones();
    assert!(left.doc.sync.tombstones.contains_key(&key));
    assert!(left.sync_state().tombstone_acks.is_empty());
    // A finished session records the ack, and then it may go.
    left.sync_ack_tombstones("right-id", &theirs).unwrap();
    assert!(left.sync_state().tombstone_acks["right-id"] > 0);
    left.compact_tombstones();
    assert!(!left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn a_tombstone_is_compacted_only_after_every_peer_has_it() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 200);
    left.sync_trust_peer("dev-b", "Phone", "bb").unwrap();
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    let has_it = left.sync_manifest(None).unwrap();
    let lacks_it = Manifest::new();
    // Both exchanged with us (the old rule would drop the deletion now).
    left.sync_mark_synced("dev-b", None).unwrap();
    left.sync_mark_synced("dev-c", None).unwrap();
    left.sync_ack_tombstones("dev-b", &has_it).unwrap();
    left.sync_ack_tombstones("dev-c", &lacks_it).unwrap();
    left.compact_tombstones();
    assert!(left.doc.sync.tombstones.contains_key(&key));
    left.sync_ack_tombstones("dev-c", &has_it).unwrap();
    left.compact_tombstones();
    assert!(!left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn a_young_tombstone_stays_even_when_everyone_has_it() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 10);
    left.sync_trust_peer("dev-b", "Phone", "bb").unwrap();
    let has_it = left.sync_manifest(None).unwrap();
    left.sync_ack_tombstones("dev-b", &has_it).unwrap();
    left.compact_tombstones();
    assert!(left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn an_ack_never_goes_down() {
    let (_dir, mut left, mut right) = two_devices();
    deleted_long_ago(&mut left, &mut right, 200);
    left.sync_trust_peer("dev-b", "Phone", "bb").unwrap();
    let has_it = left.sync_manifest(None).unwrap();
    left.sync_ack_tombstones("dev-b", &has_it).unwrap();
    let acked = left.sync_state().tombstone_acks["dev-b"];
    left.sync_ack_tombstones("dev-b", &Manifest::new()).unwrap();
    assert_eq!(left.sync_state().tombstone_acks["dev-b"], acked);
}

#[test]
fn a_forgotten_device_repaired_within_a_year_does_not_bring_deletions_back() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 200);
    // A third device has the deletion and told us so.
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    let has_it = left.sync_manifest(None).unwrap();
    left.sync_mark_synced("dev-c", None).unwrap();
    left.sync_ack_tombstones("dev-c", &has_it).unwrap();
    // The right device is forgotten 100 days ago and never saw the deletion.
    left.sync_trust_peer("right-id", "Right", "rr").unwrap();
    left.sync_forget_peer("right-id").unwrap();
    age_revocation(&mut left, "right-id", 100);
    left.compact_tombstones();
    assert!(
        left.doc.sync.tombstones.contains_key(&key),
        "it still gates"
    );
    // Paired again, it still holds the contact: the deletion wins.
    left.sync_trust_peer("right-id", "Right", "rr").unwrap();
    exchange(&mut left, &mut right);
    assert!(left.doc.contacts.is_empty());
    assert!(right.doc.contacts.is_empty());
}

#[test]
fn a_device_forgotten_over_a_year_ago_no_longer_holds_compaction() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 500);
    left.sync_trust_peer("right-id", "Right", "rr").unwrap();
    left.sync_forget_peer("right-id").unwrap();
    age_revocation(&mut left, "right-id", 400);
    left.compact_tombstones();
    assert!(!left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn a_compacted_tombstone_from_a_peer_makes_no_batch() {
    let (_dir, mut left, _right) = two_devices();
    let key = "contacts:compacted".to_string();
    let incoming: BTreeMap<String, RecordState> = [(
        key.clone(),
        RecordState {
            value: None,
            stamps: None,
            tombstone: Some(Stamp::new(days_ago(400), 0, "bbbbbbbb")),
        },
    )]
    .into();
    let batches = left.doc.sync.batches.len();
    let journal = left.doc.sync.journal.len();
    assert!(
        left.sync_apply(&incoming, &peer("right"))
            .unwrap()
            .is_none()
    );
    assert!(!left.doc.sync.tombstones.contains_key(&key));
    assert!(!left.doc.sync.tombstones_seen.contains_key(&key));
    assert_eq!(left.doc.sync.batches.len(), batches);
    assert_eq!(left.doc.sync.journal.len(), journal);
}

#[test]
fn a_recent_tombstone_from_a_peer_is_still_taken() {
    let (_dir, mut left, _right) = two_devices();
    let key = "contacts:fresh".to_string();
    let incoming: BTreeMap<String, RecordState> = [(
        key.clone(),
        RecordState {
            value: None,
            stamps: None,
            tombstone: Some(Stamp::new(days_ago(10), 0, "bbbbbbbb")),
        },
    )]
    .into();
    assert!(
        left.sync_apply(&incoming, &peer("right"))
            .unwrap()
            .is_some()
    );
    assert!(left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn an_old_peer_without_acks_does_not_resurrect_a_deletion() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 200);
    left.sync_trust_peer("right-id", "Right", "rr").unwrap();
    // The old peer still holds the live contact and sends the same manifest
    // as ever; it never vouches for the deletion.
    let theirs = right.sync_manifest(None).unwrap();
    left.sync_ack_tombstones("right-id", &theirs).unwrap();
    left.sync_mark_synced("right-id", None).unwrap();
    left.compact_tombstones();
    assert!(left.doc.sync.tombstones.contains_key(&key));
    exchange(&mut left, &mut right);
    assert!(left.doc.contacts.is_empty());
    assert!(right.doc.contacts.is_empty());
}

#[test]
fn a_revocation_of_this_device_stays_in_the_list_it_passes_on() {
    let (_dir, mut left, mut right) = two_devices();
    right
        .sync_trust_peer("aaaaaaaa-left", "Left", "aa")
        .unwrap();
    right.sync_forget_peer("aaaaaaaa-left").unwrap();
    left.sync_merge_peers(&right.sync_state().peers.clone())
        .unwrap();
    assert!(left.sync_state().is_revoked("aa"));
}

#[test]
fn an_own_entry_already_in_the_peer_list_does_not_hold_compaction() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 200);
    left.sync_trust_peer("bbbbbbbb-right", "Right", "bb")
        .unwrap();
    // A vault written before the fix: this device sits in its own list.
    left.sync_trust_peer("aaaaaaaa-left", "Left", "aa").unwrap();
    let has_it = left.sync_manifest(None).unwrap();
    left.sync_mark_synced("bbbbbbbb-right", None).unwrap();
    left.sync_ack_tombstones("bbbbbbbb-right", &has_it).unwrap();
    left.compact_tombstones();
    assert!(!left.doc.sync.tombstones.contains_key(&key));
}
