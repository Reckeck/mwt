//! The clock follows no stamp from beyond the skew limit, so a device with a
//! wrong date cannot push a revocation out of every peer's reach.

use super::tests::{exchange, two_devices};
use super::*;
use crate::household::contacts::ContactDraft;
use crate::model::contact::ContactKind;
use crate::sync::stamp::{MAX_FUTURE_MS, now_ms};

const DAY_MS: u64 = 24 * 3600 * 1000;

fn limit() -> u64 {
    now_ms() + MAX_FUTURE_MS
}

/// A contact whose every stamp lies thirty days ahead (a device with a wrong
/// date made it). Returns the record key.
fn poisoned_contact(vault: &mut UnlockedVault) -> String {
    let contact = crate::usecases::household::contacts::create_contact(
        vault,
        ContactDraft {
            name: "From the future".into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        },
    )
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
    // The record arrived, its poisoned stamp stored lowered ...
    assert!(right.doc.sync.stamps.contains_key(&key));
    assert!(right.doc.sync.stamps[&key].created.wall <= limit());
    // ... but the clock did not follow it.
    crate::usecases::sync::trust::sync_trust_peer(&mut right, "dev-c", "Tablet", "cc").unwrap();
    let stamp = &right.sync_state().trusted("cc").unwrap().stamp;
    assert!(stamp.wall <= limit(), "clock followed a future stamp");
}

#[test]
fn a_revocation_after_a_future_record_reaches_the_other_device() {
    let (_dir, mut left, mut right) = two_devices();
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-c", "Tablet", "cc").unwrap();
    right
        .sync_merge_peers(&left.sync_state().peers.clone())
        .unwrap();
    poisoned_contact(&mut right);
    exchange(&mut left, &mut right);
    crate::usecases::sync::trust::sync_forget_peer(&mut left, "dev-c").unwrap();
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
    crate::usecases::sync::trust::sync_trust_peer(&mut reopened, "dev-c", "Tablet", "cc").unwrap();
    let stamp = &reopened.sync_state().trusted("cc").unwrap().stamp;
    assert!(stamp.wall <= limit(), "reopened clock is poisoned");
}

#[test]
fn a_poisoned_own_revocation_is_restamped_before_sending() {
    let (_dir, mut left, mut right) = two_devices();
    crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-c", "Tablet", "cc").unwrap();
    right
        .sync_merge_peers(&left.sync_state().peers.clone())
        .unwrap();
    crate::usecases::sync::trust::sync_forget_peer(&mut left, "dev-c").unwrap();
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
    crate::usecases::sync::trust::sync_trust_peer(&mut right, "dev-c", "Tablet", "cc").unwrap();
    let older_trust = right.sync_state().peers.clone();
    crate::usecases::sync::trust::sync_forget_peer(&mut right, "dev-c").unwrap();
    let newer_revocation = right.sync_state().peers.clone();
    third.sync_merge_peers(&newer_revocation).unwrap();
    third.sync_merge_peers(&older_trust).unwrap();
    assert!(third.sync_state().is_revoked("cc"));
    assert!(third.sync_state().trusted("cc").is_none());
}

#[test]
fn an_edit_of_a_record_poisoned_beyond_the_limit_is_stamped_now() {
    let (_dir, mut left, mut right) = two_devices();
    let key = poisoned_contact(&mut left);
    exchange(&mut left, &mut right);
    let id = key.trim_start_matches("contacts:").to_string();
    let draft = ContactDraft {
        name: "Edited on the right".into(),
        kind: ContactKind::Person,
        ..ContactDraft::default()
    };
    crate::usecases::household::contacts::update_contact(&mut right, &id, draft).unwrap();
    // The poisoned stamp is thirty days ahead: the edit is stamped now (the
    // old rule gave one past it, which peers skip when they follow a clock).
    let stamp = right.doc.sync.stamps[&key].field("name").clone();
    assert!(stamp.wall <= limit(), "the edit followed a poisoned stamp");
    assert_eq!(stamp.device, "bbbbbbbb");
    exchange(&mut left, &mut right);
    // The poisoned stamp on the left must not beat the edit from now.
    let name_of = |vault: &UnlockedVault| {
        vault
            .doc
            .contacts
            .iter()
            .find(|contact| contact.id == id)
            .unwrap()
            .name
            .clone()
    };
    assert_eq!(name_of(&left), "Edited on the right");
    assert_eq!(name_of(&right), "Edited on the right");
}

fn has_contact(vault: &UnlockedVault, id: &str) -> bool {
    vault.doc.contacts.iter().any(|contact| contact.id == id)
}

fn plain_contact(vault: &mut UnlockedVault) -> crate::model::contact::Contact {
    crate::usecases::household::contacts::create_contact(
        vault,
        ContactDraft {
            name: "Plain".into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        },
    )
    .unwrap()
}

#[test]
fn deleting_a_poisoned_record_sticks() {
    let (_dir, mut left, mut right) = two_devices();
    let key = poisoned_contact(&mut left);
    let id = key.trim_start_matches("contacts:").to_string();
    exchange(&mut left, &mut right);
    crate::usecases::household::contacts::delete_contact(&mut right, &id).unwrap();
    exchange(&mut left, &mut right);
    exchange(&mut left, &mut right);
    assert!(!has_contact(&left, &id), "the deleted record came back");
    assert!(!has_contact(&right, &id));
}

#[test]
fn recreating_over_a_poisoned_tombstone_survives() {
    let (_dir, mut left, mut right) = two_devices();
    let contact = plain_contact(&mut left);
    let key = format!("contacts:{}", contact.id);
    exchange(&mut left, &mut right);
    crate::usecases::household::contacts::delete_contact(&mut left, &contact.id).unwrap();
    left.doc
        .sync
        .tombstones
        .insert(key, Stamp::new(now_ms() + 30 * DAY_MS, 0, "aaaaaaaa"));
    left.persist().unwrap();
    exchange(&mut left, &mut right);
    assert!(!has_contact(&right, &contact.id));
    right.doc.contacts.push(contact.clone());
    right.persist().unwrap();
    exchange(&mut left, &mut right);
    exchange(&mut left, &mut right);
    assert!(
        has_contact(&left, &contact.id),
        "the recreated record was erased"
    );
    assert!(has_contact(&right, &contact.id));
}

fn name_of(vault: &UnlockedVault, id: &str) -> String {
    vault
        .doc
        .contacts
        .iter()
        .find(|contact| contact.id == id)
        .unwrap()
        .name
        .clone()
}

fn rename(vault: &mut UnlockedVault, id: &str, name: &str) {
    let draft = ContactDraft {
        name: name.into(),
        kind: ContactKind::Person,
        ..ContactDraft::default()
    };
    crate::usecases::household::contacts::update_contact(vault, id, draft).unwrap();
}

#[test]
fn an_edit_made_in_the_poisoned_window_survives_after_it() {
    let (_dir, mut left, mut right) = two_devices();
    let key = poisoned_contact(&mut left);
    let id = key.trim_start_matches("contacts:").to_string();
    exchange(&mut left, &mut right);
    rename(&mut right, &id, "Edited in the window");
    exchange(&mut left, &mut right);
    exchange(&mut left, &mut right);
    // The merge stored every poisoned stamp lowered, so none of them can
    // become valid (and beat the edit) once it ages.
    for vault in [&left, &right] {
        let stamps = &vault.doc.sync.stamps[&key];
        assert!(stamps.created.wall <= limit(), "created stayed poisoned");
        assert!(stamps.fields.values().all(|stamp| stamp.wall <= limit()));
    }
    assert_eq!(name_of(&left, &id), "Edited in the window");
    assert_eq!(name_of(&right, &id), "Edited in the window");
}

#[test]
fn two_devices_with_twelve_hours_of_clock_skew_converge() {
    let (_dir, mut left, mut right) = two_devices();
    let contact = plain_contact(&mut left);
    let key = format!("contacts:{}", contact.id);
    // The left clock runs twelve hours ahead: inside the limit, not poisoned.
    let ahead = now_ms() + 12 * 3600 * 1000;
    let stamps = left.doc.sync.stamps.get_mut(&key).unwrap();
    stamps.created = Stamp::new(ahead, 0, "aaaaaaaa");
    for stamp in stamps.fields.values_mut() {
        *stamp = Stamp::new(ahead, 0, "aaaaaaaa");
    }
    left.persist().unwrap();
    exchange(&mut left, &mut right);
    rename(&mut right, &contact.id, "Right edit");
    rename(&mut left, &contact.id, "Left edit");
    exchange(&mut left, &mut right);
    exchange(&mut left, &mut right);
    assert_eq!(
        name_of(&left, &contact.id),
        name_of(&right, &contact.id),
        "the devices diverged"
    );
}
