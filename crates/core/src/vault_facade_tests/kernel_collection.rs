//! Tests of `Collection<T>` that commit through `UnlockedVault`: it lives in the facade, so
//! these moved here from `mwt-vault` with their assertions unchanged.

use std::sync::Arc;

use mwt_vault::kernel::session::SaveHook;

use crate::model::contact::Contact;
use crate::model::vault::ImportRun;
use crate::service::{FlushJob, UnlockedVault};

struct NoWake;

impl SaveHook for NoWake {
    fn dirty(&self) {}
}

fn contact(id: &str) -> Contact {
    Contact {
        id: id.into(),
        name: "Pilot".into(),
        ..Contact::default()
    }
}

fn run(id: &str) -> ImportRun {
    ImportRun {
        id: id.into(),
        ..ImportRun::default()
    }
}

fn batched_vault() -> (tempfile::TempDir, UnlockedVault) {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
    vault.set_device("aaaaaaaa-collection");
    vault.set_write_behind(Some(Arc::new(NoWake)));
    (dir, vault)
}

fn flush(vault: &mut UnlockedVault) {
    let job: Option<FlushJob> = vault.prepare_flush().unwrap();
    if let Some(job) = job {
        let done = job.write().unwrap();
        vault.finish_flush(&done);
    }
}

fn has_key(keys: impl IntoIterator<Item = impl AsRef<str>>, id: &str) -> bool {
    keys.into_iter().any(|key| key.as_ref().contains(id))
}

#[test]
fn a_record_removed_through_the_collection_gets_a_tombstone_in_the_next_save() {
    let (_dir, mut vault) = batched_vault();
    vault
        .transact(|tx| tx.collection::<Contact>().insert(contact("gone-1")))
        .unwrap();
    flush(&mut vault);
    assert!(has_key(vault.doc.sync.stamps.keys(), "gone-1"));
    vault
        .transact(|tx| tx.collection::<Contact>().remove("gone-1").map(|_| ()))
        .unwrap();
    flush(&mut vault);
    assert!(has_key(vault.doc.sync.tombstones.keys(), "gone-1"));
    assert!(!has_key(vault.doc.sync.stamps.keys(), "gone-1"));
}

#[test]
fn a_log_record_removed_through_the_collection_is_announced_without_a_trim() {
    let (_dir, mut vault) = batched_vault();
    vault
        .transact(|tx| tx.collection::<ImportRun>().insert(run("run-1")))
        .unwrap();
    flush(&mut vault);
    vault
        .transact(|tx| tx.collection::<ImportRun>().remove("run-1").map(|_| ()))
        .unwrap();
    assert!(
        has_key(vault.doc.sync.tombstones.keys(), "run-1"),
        "the removal is a deletion, not a trim"
    );
    assert!(!has_key(vault.doc.sync.stamps.keys(), "run-1"));
    flush(&mut vault);
    assert!(has_key(vault.doc.sync.tombstones.keys(), "run-1"));
}
