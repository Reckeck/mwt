//! A record that does not fit the schema is skipped on its own: the good
//! records of the batch apply, the session goes on, and the skip is told once.

use std::collections::BTreeMap;

use super::tests::two_devices;
use super::*;
use crate::model::contact::ContactKind;
use crate::service::contacts::ContactDraft;

fn contact(vault: &mut UnlockedVault, name: &str) -> String {
    let created = vault
        .create_contact(ContactDraft {
            name: name.into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        })
        .unwrap();
    format!("contacts:{}", created.id)
}

fn peer() -> PeerRef {
    PeerRef {
        device_id: "right-id".into(),
        name: "right".into(),
    }
}

/// What `right` has and `left` lacks, with the record `bad` broken.
fn incoming_with(
    left: &mut UnlockedVault,
    right: &mut UnlockedVault,
    bad: &str,
) -> BTreeMap<String, RecordState> {
    let mine = left.sync_manifest(None).unwrap();
    let theirs = right.sync_manifest(None).unwrap();
    let (wanted, _) = left.sync_diff(&mine, &theirs);
    let mut incoming = right.sync_records(&wanted).unwrap();
    let value = incoming
        .get_mut(bad)
        .and_then(|state| state.value.as_mut())
        .expect("record");
    value["kind"] = serde_json::json!("not-a-kind");
    incoming
}

#[test]
fn a_record_that_does_not_parse_is_skipped_and_the_rest_applies() {
    let (_dir, mut left, mut right) = two_devices();
    let good = contact(&mut right, "Good");
    let bad = contact(&mut right, "Bad");
    let incoming = incoming_with(&mut left, &mut right, &bad);

    let applied = left
        .sync_apply(&incoming, &peer())
        .unwrap()
        .expect("the good record applied");
    assert_eq!(applied.skipped, vec![bad.clone()]);
    let known: Vec<String> = left
        .doc
        .contacts
        .iter()
        .map(|item| format!("contacts:{}", item.id))
        .collect();
    assert!(known.contains(&good));
    assert!(!known.contains(&bad));
}

#[test]
fn a_skipped_record_is_journaled_once_not_every_session() {
    let (_dir, mut left, mut right) = two_devices();
    contact(&mut right, "Good");
    let bad = contact(&mut right, "Bad");
    let incoming = incoming_with(&mut left, &mut right, &bad);

    left.sync_apply(&incoming, &peer()).unwrap();
    let failed = |vault: &UnlockedVault| {
        vault
            .sync_state()
            .journal
            .iter()
            .filter(|entry| entry.outcome == SyncJournalOutcome::Failed)
            .count()
    };
    assert_eq!(failed(&left), 1);
    // The next session offers the same records again.
    let again = left.sync_apply(&incoming, &peer()).unwrap();
    assert!(again.is_none());
    assert_eq!(failed(&left), 1);
}

#[test]
fn a_batch_of_only_bad_records_ends_without_an_error() {
    let (_dir, mut left, mut right) = two_devices();
    let bad = contact(&mut right, "Bad");
    let incoming = incoming_with(&mut left, &mut right, &bad);
    assert!(left.sync_apply(&incoming, &peer()).unwrap().is_none());
    assert!(left.doc.contacts.is_empty());
}
