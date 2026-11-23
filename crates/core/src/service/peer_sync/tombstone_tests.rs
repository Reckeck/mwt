//! Deletions live until every device that counts has them (acks from the
//! peers' manifests), a forgotten device counts for a year, and a deletion
//! that was already compacted elsewhere is not news.

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::household::contacts::ContactDraft;
use crate::model::contact::ContactKind;
use crate::sync::RecordStamps;
use crate::usecases::sync::peer_sync;

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
fn deleted_long_ago(left: &mut Session, right: &mut Session, age_days: u64) -> String {
    let contact = crate::usecases::household::contacts::create_contact(
        left,
        ContactDraft {
            name: "Gone".into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        },
    )
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
    crate::usecases::household::contacts::delete_contact(left, &contact.id).unwrap();
    age_tombstone(left, &key, age_days);
    key
}

fn age_tombstone(vault: &mut Session, key: &str, age_days: u64) {
    let old = days_ago(age_days);
    vault
        .doc
        .sync
        .tombstones
        .insert(key.to_string(), Stamp::new(old, 0, "aaaaaaaa"));
    vault.doc.sync.tombstones_seen.insert(key.to_string(), old);
}

fn age_revocation(vault: &mut Session, device_id: &str, age_days: u64) {
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
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "right-id", "Right", "rr").unwrap();
    // The peer has the deletion; the session only compared manifests and
    // was declined: nothing is recorded, the deletion stays.
    peer_sync::sync_apply(
        &mut right,
        &crate::usecases::sync::read::sync_records(&mut left, std::slice::from_ref(&key)).unwrap(),
        &peer("left"),
    )
    .unwrap();
    let theirs = crate::usecases::sync::read::sync_manifest(&mut right, None).unwrap();
    let mine = crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
    let _ = crate::usecases::sync::read::sync_diff(&left, &mine, &theirs);
    peer_sync::sync_mark_synced(&mut left, "right-id", None).unwrap();
    peer_sync::compact_tombstones(&mut left);
    assert!(left.doc.sync.tombstones.contains_key(&key));
    assert!(peer_sync::sync_state(&left).tombstone_acks.is_empty());
    // A finished session records the ack, and then it may go.
    peer_sync::sync_ack_tombstones(&mut left, "right-id", &theirs).unwrap();
    assert!(peer_sync::sync_state(&left).tombstone_acks["right-id"] > 0);
    peer_sync::compact_tombstones(&mut left);
    assert!(!left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn a_tombstone_is_compacted_only_after_every_peer_has_it() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 200);
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-b", "Phone", "bb").unwrap();
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-c", "Tablet", "cc").unwrap();
    let has_it = crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
    let lacks_it = Manifest::new();
    // Both exchanged with us (the old rule would drop the deletion now).
    peer_sync::sync_mark_synced(&mut left, "dev-b", None).unwrap();
    peer_sync::sync_mark_synced(&mut left, "dev-c", None).unwrap();
    peer_sync::sync_ack_tombstones(&mut left, "dev-b", &has_it).unwrap();
    peer_sync::sync_ack_tombstones(&mut left, "dev-c", &lacks_it).unwrap();
    peer_sync::compact_tombstones(&mut left);
    assert!(left.doc.sync.tombstones.contains_key(&key));
    peer_sync::sync_ack_tombstones(&mut left, "dev-c", &has_it).unwrap();
    peer_sync::compact_tombstones(&mut left);
    assert!(!left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn a_young_tombstone_stays_even_when_everyone_has_it() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 10);
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-b", "Phone", "bb").unwrap();
    let has_it = crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
    peer_sync::sync_ack_tombstones(&mut left, "dev-b", &has_it).unwrap();
    peer_sync::compact_tombstones(&mut left);
    assert!(left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn an_ack_never_goes_down() {
    let (_dir, mut left, mut right) = two_devices();
    deleted_long_ago(&mut left, &mut right, 200);
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-b", "Phone", "bb").unwrap();
    let has_it = crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
    peer_sync::sync_ack_tombstones(&mut left, "dev-b", &has_it).unwrap();
    let acked = peer_sync::sync_state(&left).tombstone_acks["dev-b"];
    peer_sync::sync_ack_tombstones(&mut left, "dev-b", &Manifest::new()).unwrap();
    assert_eq!(peer_sync::sync_state(&left).tombstone_acks["dev-b"], acked);
}

#[test]
fn a_forgotten_device_repaired_within_a_year_does_not_bring_deletions_back() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 200);
    // A third device has the deletion and told us so.
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-c", "Tablet", "cc").unwrap();
    let has_it = crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
    peer_sync::sync_mark_synced(&mut left, "dev-c", None).unwrap();
    peer_sync::sync_ack_tombstones(&mut left, "dev-c", &has_it).unwrap();
    // The right device is forgotten 100 days ago and never saw the deletion.
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "right-id", "Right", "rr").unwrap();
    crate::usecases::sync::trust::sync_forget_peer(&mut left, "right-id").unwrap();
    age_revocation(&mut left, "right-id", 100);
    peer_sync::compact_tombstones(&mut left);
    assert!(
        left.doc.sync.tombstones.contains_key(&key),
        "it still gates"
    );
    // Paired again, it still holds the contact: the deletion wins.
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "right-id", "Right", "rr").unwrap();
    exchange(&mut left, &mut right);
    assert!(left.doc.contacts.is_empty());
    assert!(right.doc.contacts.is_empty());
}

#[test]
fn a_device_forgotten_over_a_year_ago_no_longer_holds_compaction() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 500);
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "right-id", "Right", "rr").unwrap();
    crate::usecases::sync::trust::sync_forget_peer(&mut left, "right-id").unwrap();
    age_revocation(&mut left, "right-id", 400);
    peer_sync::compact_tombstones(&mut left);
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
        peer_sync::sync_apply(&mut left, &incoming, &peer("right"))
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
        peer_sync::sync_apply(&mut left, &incoming, &peer("right"))
            .unwrap()
            .is_some()
    );
    assert!(left.doc.sync.tombstones.contains_key(&key));
}

#[test]
fn an_old_peer_without_acks_does_not_resurrect_a_deletion() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 200);
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "right-id", "Right", "rr").unwrap();
    // The old peer still holds the live contact and sends the same manifest
    // as ever; it never vouches for the deletion.
    let theirs = crate::usecases::sync::read::sync_manifest(&mut right, None).unwrap();
    peer_sync::sync_ack_tombstones(&mut left, "right-id", &theirs).unwrap();
    peer_sync::sync_mark_synced(&mut left, "right-id", None).unwrap();
    peer_sync::compact_tombstones(&mut left);
    assert!(left.doc.sync.tombstones.contains_key(&key));
    exchange(&mut left, &mut right);
    assert!(left.doc.contacts.is_empty());
    assert!(right.doc.contacts.is_empty());
}

#[test]
fn a_revocation_of_this_device_stays_in_the_list_it_passes_on() {
    let (_dir, mut left, mut right) = two_devices();
    crate::usecases::sync::trust::sync_trust_peer(&mut right, "aaaaaaaa-left", "Left", "aa")
        .unwrap();
    crate::usecases::sync::trust::sync_forget_peer(&mut right, "aaaaaaaa-left").unwrap();
    peer_sync::sync_merge_peers(&mut left, &peer_sync::sync_state(&right).peers.clone()).unwrap();
    assert!(peer_sync::sync_state(&left).is_revoked("aa"));
}

#[test]
fn an_own_entry_already_in_the_peer_list_does_not_hold_compaction() {
    let (_dir, mut left, mut right) = two_devices();
    let key = deleted_long_ago(&mut left, &mut right, 200);
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "bbbbbbbb-right", "Right", "bb")
        .unwrap();
    // A vault written before the fix: this device sits in its own list.
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "aaaaaaaa-left", "Left", "aa")
        .unwrap();
    let has_it = crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
    peer_sync::sync_mark_synced(&mut left, "bbbbbbbb-right", None).unwrap();
    peer_sync::sync_ack_tombstones(&mut left, "bbbbbbbb-right", &has_it).unwrap();
    peer_sync::compact_tombstones(&mut left);
    assert!(!left.doc.sync.tombstones.contains_key(&key));
}
