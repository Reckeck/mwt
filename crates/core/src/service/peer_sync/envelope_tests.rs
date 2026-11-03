//! Envelope top-ups from several devices add up: the allocation is a journal
//! of moves, `allocated` is derived from it, and a device that has no journal
//! (it just writes `allocated`) becomes one correction entry.

use super::tests::{settle, two_devices};
use super::*;
use crate::model::planning::{Envelope, EnvelopeMove};
use crate::model::store::StoreKind;
use crate::service::envelopes::EnvelopeDraft;
use crate::service::test_support::{amount, income, store, usd};
use crate::sync::stamp::Stamp;

fn draft(store_id: &str) -> EnvelopeDraft {
    EnvelopeDraft {
        store_id: store_id.into(),
        name: "Trip".into(),
        asset: usd(),
        target: None,
        goal_id: None,
    }
}

/// An envelope in a funded wallet that both devices hold: `(store, envelope)`.
fn shared_envelope(left: &mut UnlockedVault, right: &mut UnlockedVault) -> (String, String) {
    let cash = store(left, "Cash", StoreKind::Cash, None);
    left.add_transaction(income(&cash, "1000", "pay")).unwrap();
    let envelope = left.create_envelope(draft(&cash)).unwrap();
    settle(left, right);
    (cash, envelope.id)
}

fn envelope_of(vault: &UnlockedVault, id: &str) -> Envelope {
    vault
        .doc
        .envelopes
        .iter()
        .find(|envelope| envelope.id == id)
        .cloned()
        .unwrap()
}

fn allocated(vault: &UnlockedVault, id: &str) -> crate::Amount {
    envelope_of(vault, id).allocated
}

/// The journal adds up to `allocated`, the stored stamp of `allocated` is the
/// stamp of the newest entry.
fn assert_derived(vault: &UnlockedVault, id: &str) {
    let envelope = envelope_of(vault, id);
    assert_eq!(envelope.allocated, envelope.moves_sum(), "allocated = sum");
    let stamps = vault
        .doc
        .sync
        .stamps
        .get(&format!("envelopes:{id}"))
        .expect("stamps");
    let newest = stamps
        .fields
        .iter()
        .filter(|(name, _)| name.starts_with("moves."))
        .map(|(_, stamp)| stamp)
        .max();
    if let Some(newest) = newest {
        assert_eq!(
            stamps.fields.get("allocated"),
            Some(newest),
            "derived stamp"
        );
    }
}

fn corrections(vault: &UnlockedVault, id: &str) -> Vec<EnvelopeMove> {
    envelope_of(vault, id)
        .moves
        .into_iter()
        .filter(|entry| entry.id.starts_with("fix:"))
        .collect()
}

fn pull(devices: &mut [UnlockedVault], from: usize, to: usize) {
    let from_manifest = devices[from].sync_manifest(None).unwrap();
    let to_manifest = devices[to].sync_manifest(None).unwrap();
    let (wants, _) = devices[to].sync_diff(&to_manifest, &from_manifest);
    let records = devices[from].sync_records(&wants).unwrap();
    let peer = PeerRef {
        device_id: format!("device-{from}-id"),
        name: format!("device-{from}"),
    };
    devices[to].sync_apply(&records, &peer).unwrap();
}

fn three_devices() -> (tempfile::TempDir, Vec<UnlockedVault>) {
    let (dir, left, right) = two_devices();
    let copy = dir.path().join("third.wallet");
    std::fs::copy(&left.path, &copy).unwrap();
    let mut third = UnlockedVault::open(copy, "pw").unwrap();
    third.set_device("cccccccc-third");
    (dir, vec![left, right, third])
}

/// What an older device sends for the envelope: no `moves`, no stamps of
/// entries; it only knows the fields it has, each with its own stamp.
fn as_old_peer(mut state: RecordState, allocated: &str, edited: Option<Stamp>) -> RecordState {
    let mut value = state.value.take().expect("value");
    value.as_object_mut().unwrap().remove("moves");
    value["allocated"] = serde_json::json!(allocated);
    let stamps = state.stamps.as_mut().expect("stamps");
    stamps.fields.retain(|name, _| !name.starts_with("moves"));
    if let Some(stamp) = edited {
        stamps.fields.insert("allocated".into(), stamp);
    }
    state.value = Some(value);
    state
}

fn later(vault: &mut UnlockedVault) -> Stamp {
    let mut stamp = vault.clock.tick();
    stamp.wall += 5_000;
    stamp.device = "oldpeer".into();
    stamp
}

#[test]
fn concurrent_top_ups_add_up() {
    let (_dir, mut left, mut right) = two_devices();
    let (cash, id) = shared_envelope(&mut left, &mut right);
    left.fund_envelope(&id, amount("10")).unwrap();
    right.fund_envelope(&id, amount("20")).unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(allocated(vault, &id), amount("30"));
        assert_eq!(envelope_of(vault, &id).moves.len(), 2);
        assert_derived(vault, &id);
    }
    // A move between envelopes on one side meets a top-up on the other.
    let other = left.create_envelope(draft(&cash)).unwrap();
    settle(&mut left, &mut right);
    left.move_between_envelopes(&id, &other.id, amount("5"))
        .unwrap();
    right.fund_envelope(&id, amount("7")).unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(allocated(vault, &id), amount("32"));
        assert_eq!(allocated(vault, &other.id), amount("5"));
    }
}

#[test]
fn three_devices_never_produce_a_false_correction() {
    // Every order of a chain of pairwise syncs.
    let orders: &[&[(usize, usize)]] = &[
        &[(0, 1), (1, 0), (0, 2), (2, 0), (1, 2), (2, 1)],
        &[(2, 0), (0, 2), (1, 0), (0, 1), (2, 1), (1, 2)],
        &[(1, 2), (2, 1), (0, 1), (1, 0), (2, 0), (0, 2)],
        &[(0, 1), (0, 2), (1, 2), (2, 1), (1, 0), (2, 0)],
    ];
    for order in orders {
        let (_dir, mut devices) = three_devices();
        let (_, id) = {
            let (head, rest) = devices.split_at_mut(1);
            shared_envelope(&mut head[0], &mut rest[0])
        };
        pull(&mut devices, 0, 2);
        devices[0].fund_envelope(&id, amount("10")).unwrap();
        devices[1].fund_envelope(&id, amount("20")).unwrap();
        devices[2].fund_envelope(&id, amount("30")).unwrap();
        for (from, to) in *order {
            pull(&mut devices, *from, *to);
            // A second top-up between sessions, on the device that just merged.
            if (*from, *to) == (1, 0) {
                devices[0].fund_envelope(&id, amount("1")).unwrap();
            }
        }
        for _ in 0..3 {
            for from in 0..3 {
                for to in 0..3 {
                    if from != to {
                        pull(&mut devices, from, to);
                    }
                }
            }
        }
        for vault in &devices {
            assert_eq!(allocated(vault, &id), amount("61"), "order {order:?}");
            assert!(corrections(vault, &id).is_empty(), "order {order:?}");
            assert_eq!(envelope_of(vault, &id).moves.len(), 4, "order {order:?}");
            assert_derived(vault, &id);
        }
    }
}

#[test]
fn opening_entry_is_shared_by_two_devices() {
    let (dir, mut left, mut right) = two_devices();
    let (_, id) = shared_envelope(&mut left, &mut right);
    // An envelope as an older version stored it: money, no journal.
    left.doc.envelopes[0].allocated = amount("40");
    left.doc.envelopes[0].moves.clear();
    left.persist().unwrap();
    let path = left.path.clone();
    drop(left);
    // The first device opens it (upgrade), the second gets the record by sync.
    let mut left = UnlockedVault::open_as(path, "pw", Some("aaaaaaaa-left")).unwrap();
    settle(&mut left, &mut right);
    let opening = vec![EnvelopeMove {
        id: format!("open:{id}"),
        delta: amount("40"),
        at: envelope_of(&left, &id).created_at,
    }];
    assert_eq!(envelope_of(&left, &id).moves, opening);
    assert_eq!(envelope_of(&right, &id).moves, opening);
    // And the same entry made by a device that opens its own copy: no news.
    let right_path = right.path.clone();
    drop(right);
    let mut right = UnlockedVault::open_as(right_path, "pw", Some("bbbbbbbb-right")).unwrap();
    assert_eq!(envelope_of(&right, &id).moves, opening);
    let key = format!("envelopes:{id}");
    assert_eq!(
        left.sync_manifest(None).unwrap()[&key],
        right.sync_manifest(None).unwrap()[&key],
        "the record is the same on both devices"
    );
    assert_derived(&left, &id);
    assert_derived(&right, &id);
    drop(dir);
}

#[test]
fn old_peer_allocated_edit_becomes_one_correction() {
    let (_dir, mut left, mut right) = two_devices();
    let (_, id) = shared_envelope(&mut left, &mut right);
    left.fund_envelope(&id, amount("10")).unwrap();
    settle(&mut left, &mut right);
    let key = format!("envelopes:{id}");
    let stamp = later(&mut left);
    let from_old = |vault: &mut UnlockedVault, allocated: &str, stamp: Stamp| {
        let mut records = vault.sync_records(std::slice::from_ref(&key)).unwrap();
        let state = records.remove(&key).unwrap();
        BTreeMap::from([(key.clone(), as_old_peer(state, allocated, Some(stamp)))])
    };
    // The old device topped it up to 25 (it saw 10).
    let batch = from_old(&mut right, "25", stamp.clone());
    right.sync_apply(&batch, &peer("old")).unwrap();
    assert_eq!(allocated(&right, &id), amount("25"));
    let fixes = corrections(&right, &id);
    assert_eq!(fixes.len(), 1);
    assert_eq!(fixes[0].delta, amount("15"));
    assert_eq!(envelope_of(&right, &id).moves.len(), 2);
    assert_derived(&right, &id);
    // Delivered again, and to the other new device: still one entry.
    right.sync_apply(&batch, &peer("old")).unwrap();
    left.sync_apply(&batch, &peer("old")).unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(corrections(vault, &id).len(), 1, "one correction");
        assert_eq!(allocated(vault, &id), amount("25"));
        assert_derived(vault, &id);
    }
    // A later edit by the old device is a second correction.
    let second = {
        let mut next = stamp.clone();
        next.wall += 1_000;
        next
    };
    let batch = from_old(&mut right, "30", second);
    left.sync_apply(&batch, &peer("old")).unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(corrections(vault, &id).len(), 2);
        assert_eq!(allocated(vault, &id), amount("30"));
    }
}

fn peer(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

#[test]
fn old_peer_value_without_moves_keeps_the_journal() {
    let (_dir, mut left, mut right) = two_devices();
    let (_, id) = shared_envelope(&mut left, &mut right);
    left.fund_envelope(&id, amount("10")).unwrap();
    right.fund_envelope(&id, amount("5")).unwrap();
    settle(&mut left, &mut right);
    let key = format!("envelopes:{id}");
    // The old device only renamed the envelope: its `allocated` is what it
    // last saw, with an old stamp, and it has no entries at all.
    let mut records = right.sync_records(std::slice::from_ref(&key)).unwrap();
    let mut state = as_old_peer(records.remove(&key).unwrap(), "10", None);
    state.value.as_mut().unwrap()["name"] = serde_json::json!("Renamed");
    let stamp = later(&mut right);
    state
        .stamps
        .as_mut()
        .unwrap()
        .fields
        .insert("name".into(), stamp);
    let before = envelope_of(&left, &id).moves;
    left.sync_apply(&BTreeMap::from([(key.clone(), state)]), &peer("old"))
        .unwrap();
    let after = envelope_of(&left, &id);
    assert_eq!(after.name, "Renamed");
    assert_eq!(after.moves, before);
    assert_eq!(after.allocated, amount("15"));
    assert!(corrections(&left, &id).is_empty());
    assert_derived(&left, &id);
}

#[test]
fn a_relayed_value_of_a_new_device_is_no_correction() {
    // An old device that merely forwards what a new one wrote carries the
    // new device's stamp for `allocated` (and for `moves`); the entries are
    // not in it. The receiver must not invent a correction.
    let (_dir, mut left, mut right) = two_devices();
    let (_, id) = shared_envelope(&mut left, &mut right);
    left.fund_envelope(&id, amount("10")).unwrap();
    let key = format!("envelopes:{id}");
    let mut records = left.sync_records(std::slice::from_ref(&key)).unwrap();
    let state = records.remove(&key).unwrap();
    let head = state.stamps.as_ref().unwrap().fields["allocated"].clone();
    let mut relayed = as_old_peer(state, "10", Some(head.clone()));
    relayed
        .stamps
        .as_mut()
        .unwrap()
        .fields
        .insert("moves".into(), head);
    right
        .sync_apply(&BTreeMap::from([(key.clone(), relayed)]), &peer("old"))
        .unwrap();
    assert!(corrections(&right, &id).is_empty());
    assert!(
        envelope_of(&right, &id).moves.is_empty(),
        "no opening entry either"
    );
    assert_eq!(allocated(&right, &id), amount("0"));
}

#[test]
fn overspent_envelope_after_a_merge_is_reported() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = store(&mut left, "Cash", StoreKind::Cash, None);
    left.add_transaction(income(&cash, "100", "pay")).unwrap();
    let envelope = left.create_envelope(draft(&cash)).unwrap();
    settle(&mut left, &mut right);
    // Each device sees 100 free and takes 60: together 120 of 100.
    left.fund_envelope(&envelope.id, amount("60")).unwrap();
    right.fund_envelope(&envelope.id, amount("60")).unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(allocated(vault, &envelope.id), amount("120"));
        let views = vault.envelopes(Some(&cash));
        assert_eq!(views.len(), 1);
        assert!(views[0].short, "reported as short");
        assert_eq!(views[0].free, amount("-20"));
    }
}

#[test]
fn a_deleted_envelope_stays_deleted() {
    let (_dir, mut left, mut right) = two_devices();
    let (_, id) = shared_envelope(&mut left, &mut right);
    left.fund_envelope(&id, amount("10")).unwrap();
    settle(&mut left, &mut right);
    // A top-up on one device, then the deletion on the other: gone for both.
    right.fund_envelope(&id, amount("20")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(3));
    left.delete_envelope(&id).unwrap();
    settle(&mut left, &mut right);
    assert!(left.doc.envelopes.is_empty());
    assert!(right.doc.envelopes.is_empty());
}

/// What an old device forwards of a new device's envelope: the value without
/// `moves`, `allocated` with the new device's stamp and no stamp of the array.
fn stripped_relay(from: &mut UnlockedVault, key: &str, allocated: &str) -> RecordState {
    let mut records = from
        .sync_records(std::slice::from_ref(&key.to_string()))
        .unwrap();
    let state = records.remove(key).unwrap();
    let stamp = state.stamps.as_ref().unwrap().fields["allocated"].clone();
    as_old_peer(state, allocated, Some(stamp))
}

#[test]
fn a_correction_from_a_stripped_relay_is_withdrawn_when_the_real_move_arrives() {
    let (_dir, mut devices) = three_devices();
    let (_, id) = {
        let (head, rest) = devices.split_at_mut(1);
        shared_envelope(&mut head[0], &mut rest[0])
    };
    pull(&mut devices, 0, 2);
    let key = format!("envelopes:{id}");
    devices[0].fund_envelope(&id, amount("10")).unwrap();
    let relay = stripped_relay(&mut devices[0], &key, "10");
    // C hears it from the old device first: a (false) correction.
    devices[2]
        .sync_apply(
            &BTreeMap::from([(key.clone(), relay.clone())]),
            &peer("old"),
        )
        .unwrap();
    assert_eq!(allocated(&devices[2], &id), amount("10"));
    // B holds the correction too (the same relay).
    devices[1]
        .sync_apply(&BTreeMap::from([(key.clone(), relay)]), &peer("old"))
        .unwrap();
    assert_eq!(corrections(&devices[1], &id).len(), 1);
    // The real move arrives.
    pull(&mut devices, 0, 2);
    assert_eq!(allocated(&devices[2], &id), amount("10"));
    assert!(corrections(&devices[2], &id).is_empty());
    assert_derived(&devices[2], &id);
    // The device that still holds the correction converges to the same state.
    pull(&mut devices, 2, 1);
    pull(&mut devices, 0, 1);
    pull(&mut devices, 1, 2);
    pull(&mut devices, 1, 0);
    for vault in &devices {
        assert_eq!(allocated(vault, &id), amount("10"));
        assert!(corrections(vault, &id).is_empty());
        assert_derived(vault, &id);
    }
    let digest = |vault: &mut UnlockedVault| vault.sync_manifest(None).unwrap()[&key];
    let first = digest(&mut devices[0]);
    assert_eq!(first, digest(&mut devices[1]));
    assert_eq!(first, digest(&mut devices[2]));
}

#[test]
fn a_real_old_edit_stays_after_later_moves() {
    let (_dir, mut left, mut right) = two_devices();
    let (_, id) = shared_envelope(&mut left, &mut right);
    left.fund_envelope(&id, amount("10")).unwrap();
    settle(&mut left, &mut right);
    let key = format!("envelopes:{id}");
    let stamp = later(&mut left);
    let mut records = right.sync_records(std::slice::from_ref(&key)).unwrap();
    let batch = BTreeMap::from([(
        key.clone(),
        as_old_peer(records.remove(&key).unwrap(), "25", Some(stamp)),
    )]);
    right.sync_apply(&batch, &peer("old")).unwrap();
    left.fund_envelope(&id, amount("3")).unwrap();
    right.fund_envelope(&id, amount("4")).unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(corrections(vault, &id).len(), 1);
        assert_eq!(allocated(vault, &id), amount("32"));
        assert_derived(vault, &id);
    }
}
