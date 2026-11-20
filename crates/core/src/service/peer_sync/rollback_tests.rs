//! A sync batch that fails after the merged document went in puts everything
//! back: the document, the change tracker, and the batch's own error.

use super::tests::two_devices;
use super::*;

fn peer(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

#[test]
fn a_failed_batch_leaves_document_and_tracker_as_they_were() {
    let (_dir, mut left, mut right) = two_devices();
    right
        .create_contact(crate::service::contacts::ContactDraft {
            name: "Theirs".into(),
            kind: crate::model::contact::ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
            group: String::new(),
        })
        .unwrap();
    let right_manifest = right.sync_manifest(None).unwrap();
    let left_manifest = left.sync_manifest(None).unwrap();
    let (wants, _) = left.sync_diff(&left_manifest, &right_manifest);
    let records = right.sync_records(&wants).unwrap();

    left.track_changes().unwrap();
    let doc_before = serde_json::to_vec(&left.doc).unwrap();
    // The save at the end of the batch cannot seal without a key.
    left.key = None;

    let err = left.sync_apply(&records, &peer("right")).unwrap_err();

    assert!(matches!(err, Error::Crypto(_)), "the batch error: {err:?}");
    assert_eq!(serde_json::to_vec(&left.doc).unwrap(), doc_before);
    // A tracker still equal to the document sees no change to stamp; one left
    // at the merged state would report the rolled-back records.
    assert!(left.track_changes().unwrap().is_empty());
    assert_eq!(serde_json::to_vec(&left.doc).unwrap(), doc_before);
}

#[test]
fn swapping_the_document_cannot_fail() {
    let (_dir, mut left, _right) = two_devices();
    let mut replacement = left.doc.clone();
    replacement.sync.tombstones_seen.insert("marker".into(), 1);
    let old: VaultDoc = left.swap_doc(replacement);
    assert!(!old.sync.tombstones_seen.contains_key("marker"));
    assert!(left.doc.sync.tombstones_seen.contains_key("marker"));
}
