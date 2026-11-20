//! A sync batch is one atomic change: a step that fails after the merge was
//! swapped in leaves the document as it was.

use crate::model::store::StoreKind;
use crate::service::PeerRef;
use crate::service::test_support::empty_vault;
use crate::service::{StoreExtras, UnlockedVault};

fn peer() -> PeerRef {
    PeerRef {
        device_id: "aaaaaaaa-left".into(),
        name: "left".into(),
    }
}

/// Two devices of one vault; `left` has a store `right` does not.
fn left_ahead() -> (tempfile::TempDir, UnlockedVault, UnlockedVault) {
    let (dir, mut left) = empty_vault();
    left.set_device("aaaaaaaa-left");
    left.persist().unwrap();
    let copy = dir.path().join("copy.wallet");
    std::fs::copy(dir.path().join("test.wallet"), &copy).unwrap();
    let mut right = UnlockedVault::open(copy, "pw").unwrap();
    right.set_device("bbbbbbbb-right");
    left.create_store_with(
        "Savings jar",
        StoreKind::Bank,
        "#000",
        "",
        None,
        StoreExtras::default(),
    )
    .unwrap();
    (dir, left, right)
}

fn incoming_for(
    left: &mut UnlockedVault,
    right: &mut UnlockedVault,
) -> std::collections::BTreeMap<String, crate::sync::RecordState> {
    let mine = left.sync_manifest(None).unwrap();
    let theirs = right.sync_manifest(None).unwrap();
    let (_, right_wants) = left.sync_diff(&mine, &theirs);
    let (wants, _) = right.sync_diff(&theirs, &mine);
    let keys = if wants.is_empty() { right_wants } else { wants };
    left.sync_records(&keys).unwrap()
}

/// The document and its sync bookkeeping, serialized.
fn bytes(vault: &mut UnlockedVault) -> String {
    let doc = serde_json::to_string(&vault.doc_value().unwrap()).unwrap();
    doc + &serde_json::to_string(vault.sync_state()).unwrap()
}

#[test]
fn a_batch_that_fails_at_its_last_step_leaves_the_document_as_it_was() {
    let (dir, mut left, mut right) = left_ahead();
    let incoming = incoming_for(&mut left, &mut right);
    assert!(!incoming.is_empty());
    // The write is the last step: make it impossible.
    let file = dir.path().join("copy.wallet");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    let before = bytes(&mut right);

    let outcome = right.sync_apply(&incoming, &peer());

    assert!(outcome.is_err(), "the write was expected to fail");
    assert!(bytes(&mut right) == before, "the document changed");
}

#[test]
fn a_failed_batch_leaves_no_stamp_or_tombstone_once_the_file_is_back() {
    let (dir, mut left, mut right) = left_ahead();
    let incoming = incoming_for(&mut left, &mut right);
    let manifest = right.sync_manifest(None).unwrap();
    let file = dir.path().join("copy.wallet");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert!(right.sync_apply(&incoming, &peer()).is_err());

    // The tracker went back with the document: a later save must not read the
    // failed batch as a local change.
    std::fs::remove_dir(&file).unwrap();
    std::fs::copy(dir.path().join("test.wallet"), &file).unwrap();
    right.persist().unwrap();

    assert_eq!(right.sync_manifest(None).unwrap(), manifest);
}

#[test]
fn a_batch_that_succeeds_is_applied() {
    let (_dir, mut left, mut right) = left_ahead();
    let incoming = incoming_for(&mut left, &mut right);
    let before = bytes(&mut right);

    let applied = right.sync_apply(&incoming, &peer()).unwrap();

    assert!(applied.is_some());
    assert_ne!(bytes(&mut right), before);
}

#[derive(Default)]
struct Wakes(std::sync::atomic::AtomicUsize);

impl crate::kernel::session::SaveHook for Wakes {
    fn dirty(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn a_batch_that_succeeds_writes_once() {
    let (_dir, mut left, mut right) = left_ahead();
    let incoming = incoming_for(&mut left, &mut right);
    let wakes = std::sync::Arc::new(Wakes::default());
    right.set_write_behind(Some(wakes.clone()));

    right.sync_apply(&incoming, &peer()).unwrap();

    assert_eq!(wakes.0.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[derive(Default)]
struct Announced(std::sync::Mutex<Vec<String>>);

impl crate::service::SyncTap for Announced {
    fn changed(&self, keys: &[String]) {
        self.0.lock().unwrap().extend(keys.iter().cloned());
    }
}

#[test]
fn a_failed_batch_announces_nothing_it_stamped_on_its_way() {
    use crate::model::transaction::{Transaction, TxOrigin};
    let (dir, mut left, mut right) = left_ahead();
    let tap = std::sync::Arc::new(Announced::default());
    right.set_sync_tap(Some(tap.clone()));
    right.set_write_behind(Some(std::sync::Arc::new(Wakes::default())));
    // Two copies of one generated operation: the batch's dedupe deletes one
    // and queues its key for the live session.
    let cash = right.doc_value().unwrap()["stores"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    for comment in ["first", "second"] {
        let mut tx = crate::service::test_support::income(&cash, "100", comment);
        if let Transaction::Topup(topup) = &mut tx {
            topup.meta.origin = TxOrigin::Import {
                exec_id: "exec-1".into(),
            };
        }
        right.add_transaction(tx).unwrap();
    }
    let incoming = incoming_for(&mut left, &mut right);
    // What was stamped so far goes out first: the queue is empty.
    drop(right.prepare_flush().unwrap().expect("dirty"));
    tap.0.lock().unwrap().clear();
    right.set_write_behind(None);
    let file = dir.path().join("copy.wallet");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert!(right.sync_apply(&incoming, &peer()).is_err());

    right.set_write_behind(Some(std::sync::Arc::new(Wakes::default())));
    right.persist().unwrap();
    drop(right.prepare_flush().unwrap());

    let sent = tap.0.lock().unwrap().clone();
    assert!(sent.is_empty(), "announced after a failed batch: {sent:?}");
}
