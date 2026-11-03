//! The clock follows no stamp from beyond the skew limit, so a device with a
//! wrong date cannot push a revocation out of every peer's reach.

use super::tests::{exchange, two_devices};
use super::*;
use crate::model::contact::ContactKind;
use crate::service::contacts::ContactDraft;
use crate::sync::stamp::{MAX_FUTURE_MS, now_ms};

const DAY_MS: u64 = 24 * 3600 * 1000;

fn limit() -> u64 {
    now_ms() + MAX_FUTURE_MS
}

/// A contact whose every stamp lies thirty days ahead (a device with a wrong
/// date made it). Returns the record key.
fn poisoned_contact(vault: &mut UnlockedVault) -> String {
    let contact = vault
        .create_contact(ContactDraft {
            name: "From the future".into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        })
        .unwrap();
    let key = format!("contacts:{}", contact.id);
    let far = now_ms() + 30 * DAY_MS;
    let stamps = vault.doc.sync.stamps.get_mut(&key).unwrap();
    stamps.created = Stamp::new(far, 0, "aaaaaaaa");
    for stamp in stamps.fields.values_mut() {
        *stamp = Stamp::new(far, 0, "aaaaaaaa");
    }
    vault.persist().unwrap();
    key
}

#[test]
fn a_future_stamp_does_not_poison_the_clock() {
    let (_dir, mut left, mut right) = two_devices();
    let key = poisoned_contact(&mut left);
    exchange(&mut left, &mut right);
    // The record arrived with its stamp ...
    assert!(right.doc.sync.stamps[&key].created.wall > limit());
    // ... but the clock did not follow it.
    right.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    let stamp = &right.sync_state().trusted("cc").unwrap().stamp;
    assert!(stamp.wall <= limit(), "clock followed a future stamp");
}

#[test]
fn a_revocation_after_a_future_record_reaches_the_other_device() {
    let (_dir, mut left, mut right) = two_devices();
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    right
        .sync_merge_peers(&left.sync_state().peers.clone())
        .unwrap();
    poisoned_contact(&mut right);
    exchange(&mut left, &mut right);
    left.sync_forget_peer("dev-c").unwrap();
    let sent = left.sync_peers_to_send().unwrap();
    right.sync_merge_peers(&sent).unwrap();
    assert!(right.sync_state().is_revoked("cc"));
}

#[test]
fn a_poisoned_vault_reopens_with_a_bounded_clock() {
    let (_dir, mut left, _right) = two_devices();
    poisoned_contact(&mut left);
    let path = left.path.clone();
    drop(left);
    let mut reopened = UnlockedVault::open(path, "pw").unwrap();
    reopened.set_device("aaaaaaaa-left");
    reopened.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    let stamp = &reopened.sync_state().trusted("cc").unwrap().stamp;
    assert!(stamp.wall <= limit(), "reopened clock is poisoned");
}

#[test]
fn a_poisoned_own_revocation_is_restamped_before_sending() {
    let (_dir, mut left, mut right) = two_devices();
    left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    right
        .sync_merge_peers(&left.sync_state().peers.clone())
        .unwrap();
    left.sync_forget_peer("dev-c").unwrap();
    let far = Stamp::new(now_ms() + 30 * DAY_MS, 0, "aaaaaaaa");
    let entry = &mut left.doc.sync.peers[0];
    entry.stamp = far.clone();
    entry.revoked = Some(far);
    let sent = left.sync_peers_to_send().unwrap();
    assert!(sent[0].stamp.wall <= limit());
    assert!(sent[0].revoked.is_some());
    right.sync_merge_peers(&sent).unwrap();
    assert!(right.sync_state().is_revoked("cc"));
}

#[test]
fn a_newer_revocation_beats_an_older_trust_from_a_third_device() {
    let (_dir, mut third, mut right) = two_devices();
    right.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
    let older_trust = right.sync_state().peers.clone();
    right.sync_forget_peer("dev-c").unwrap();
    let newer_revocation = right.sync_state().peers.clone();
    third.sync_merge_peers(&newer_revocation).unwrap();
    third.sync_merge_peers(&older_trust).unwrap();
    assert!(third.sync_state().is_revoked("cc"));
    assert!(third.sync_state().trusted("cc").is_none());
}
