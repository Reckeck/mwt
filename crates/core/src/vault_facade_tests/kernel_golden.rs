//! Pins of the write path (`persist`, the batched writer, the mirror, the
//! sync stamps). They describe what the code does today and must stay green,
//! unchanged, while the write state moves into the kernel.
//!
//! The mirror counts the writes: it is pushed once per write that reached
//! the file, and never for a job the gate dropped.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::household::contacts::ContactDraft;
use crate::kernel::persistence::Flushed;
use crate::service::{FlushJob, SyncTap, UnlockedVault};
use mwt_model::{MAX_NOTICE_LOG, NoticeCategory, NoticeRecord};
use mwt_vault::kernel::session::SaveHook;
use mwt_vault::vault_io::VaultMirror;

/// Every sealed file the vault handed to the mirror, in order.
#[derive(Default)]
struct RecordingMirror(Mutex<Vec<Vec<u8>>>);

impl RecordingMirror {
    fn pushes(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    fn last(&self) -> Vec<u8> {
        self.0.lock().unwrap().last().cloned().unwrap_or_default()
    }
}

impl VaultMirror for RecordingMirror {
    fn push(&self, sealed: &[u8]) {
        self.0.lock().unwrap().push(sealed.to_vec());
    }
}

/// Counts the wake-ups instead of running a writer thread.
#[derive(Default)]
struct Wakes(AtomicUsize);

impl SaveHook for Wakes {
    fn dirty(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Every call of the sync tap, one entry per announcement.
#[derive(Default)]
struct Announcements(Mutex<Vec<Vec<String>>>);

impl SyncTap for Announcements {
    fn changed(&self, keys: &[String]) {
        self.0.lock().unwrap().push(keys.to_vec());
    }
}

impl Announcements {
    fn calls(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    /// How many announcements carried a key of the record with this id.
    fn about(&self, id: &str) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .filter(|key| key.contains(id))
            .count()
    }
}

fn open_vault() -> (tempfile::TempDir, UnlockedVault) {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
    vault.set_device("aaaaaaaa-golden");
    (dir, vault)
}

fn flush(vault: &mut UnlockedVault) -> Option<Flushed> {
    let job: FlushJob = vault.prepare_flush().unwrap()?;
    let done = job.write().unwrap();
    vault.finish_flush(&done);
    Some(done)
}

fn contact(name: &str) -> ContactDraft {
    ContactDraft {
        name: name.into(),
        ..ContactDraft::default()
    }
}

fn notice(key: &str) -> NoticeRecord {
    NoticeRecord {
        key: key.into(),
        category: NoticeCategory::Budget,
        title: key.into(),
        body: String::new(),
        link: None,
        at: "2026-11-01T10:00:00Z".into(),
        delivered_at: "2026-11-01T10:00:00Z".into(),
        read: false,
    }
}

/// The revision the next batched save would carry: one batched `persist`
/// (revision plus one), flushed, then back to write-through.
fn probe_revision(vault: &mut UnlockedVault) -> u64 {
    vault.set_write_behind(Some(Arc::new(Wakes::default())));
    vault.persist().unwrap();
    let done = flush(vault).expect("the probe left the vault dirty");
    vault.set_write_behind(None);
    done.revision
}

#[test]
fn write_through_writes_once_per_persist() {
    let (_dir, mut vault) = open_vault();
    let mirror = Arc::new(RecordingMirror::default());
    vault.set_mirror(Some(mirror.clone()));
    let first_revision = probe_revision(&mut vault);
    let pushes_before = mirror.pushes();

    let mut seen = vec![std::fs::read(&vault.path).unwrap()];
    for round in 1..=3 {
        vault.persist().unwrap();
        assert!(
            !vault.has_unsaved_changes(),
            "write-through leaves it clean"
        );
        assert_eq!(mirror.pushes(), pushes_before + round, "one write each");
        let bytes = std::fs::read(&vault.path).unwrap();
        assert!(!seen.contains(&bytes), "each write seals a new file");
        seen.push(bytes);
    }

    // `persist` bumps the revision and then `persist_now` bumps it again, so
    // a write-through persist costs two; the probe adds one more. That is the
    // observed behaviour, pinned as it is (the spec's "one" is per write).
    let probe_pushes = mirror.pushes();
    let later_revision = probe_revision(&mut vault);
    assert_eq!(later_revision - first_revision, 3 * 2 + 1);
    assert_eq!(mirror.pushes(), probe_pushes + 1, "the probe wrote once");
}

#[test]
fn a_mirror_gets_the_sealed_bytes_after_each_write() {
    let (_dir, mut vault) = open_vault();
    let mirror = Arc::new(RecordingMirror::default());
    vault.set_mirror(Some(mirror.clone()));
    assert!(vault.has_mirror());

    vault.persist().unwrap();
    assert_eq!(mirror.pushes(), 1);
    assert_eq!(mirror.last(), std::fs::read(&vault.path).unwrap());

    // Batched: nothing is pushed until the flush, then the file's bytes once.
    vault.set_write_behind(Some(Arc::new(Wakes::default())));
    for _ in 0..3 {
        vault.persist().unwrap();
    }
    assert_eq!(mirror.pushes(), 1, "a batched persist pushes nothing");
    assert!(flush(&mut vault).is_some());
    assert_eq!(mirror.pushes(), 2);
    assert_eq!(mirror.last(), std::fs::read(&vault.path).unwrap());

    // A job older than what is on disk is dropped by the gate: no push, and
    // the mirror still holds the newest file.
    vault.persist().unwrap();
    let stale = vault.prepare_flush().unwrap().unwrap();
    vault.persist().unwrap();
    vault.persist_now().unwrap();
    assert_eq!(mirror.pushes(), 3);
    stale.write().unwrap();
    assert_eq!(mirror.pushes(), 3, "a stale job pushes nothing");
    assert_eq!(mirror.last(), std::fs::read(&vault.path).unwrap());
}

#[test]
fn a_sandbox_has_no_mirror_and_pushes_nothing() {
    let (_dir, vault) = open_vault();
    let before = std::fs::read(&vault.path).unwrap();
    let mut sandbox = vault.sandbox();
    let mirror = Arc::new(RecordingMirror::default());
    sandbox.set_mirror(Some(mirror.clone()));
    assert!(!sandbox.has_mirror(), "a sandbox refuses a mirror");

    sandbox.persist().unwrap();
    sandbox.persist_now().unwrap();
    sandbox.flush_pending().unwrap();
    assert!(sandbox.prepare_flush().unwrap().is_none());
    assert!(!sandbox.has_unsaved_changes());
    drop(sandbox);

    assert_eq!(mirror.pushes(), 0);
    assert_eq!(std::fs::read(&vault.path).unwrap(), before);
}

#[test]
fn sync_stamps_are_made_at_flush_and_announced_once() {
    let (_dir, mut vault) = open_vault();
    let tap = Arc::new(Announcements::default());
    vault.set_sync_tap(Some(tap.clone()));
    vault.set_write_behind(Some(Arc::new(Wakes::default())));

    let stamped_before = vault.doc.sync.stamps.clone();
    let id = crate::usecases::household::contacts::create_contact(&mut vault, contact("Stamped"))
        .unwrap()
        .id;
    assert_eq!(
        vault.doc.sync.stamps, stamped_before,
        "persist stamps nothing"
    );
    assert_eq!(tap.calls(), 0, "persist announces nothing");

    assert!(flush(&mut vault).is_some());
    assert!(
        vault.doc.sync.stamps.keys().any(|key| key.contains(&id)),
        "the flush stamped the new record"
    );
    assert_eq!(tap.calls(), 1);
    assert_eq!(tap.about(&id), 1, "announced once");
    assert!(
        flush(&mut vault).is_none(),
        "a clean vault announces nothing"
    );
    assert_eq!(tap.calls(), 1);

    // A sync call between two saves stamps the edit itself; the next flush
    // still announces it, once.
    crate::usecases::household::contacts::archive_contact(&mut vault, &id, true).unwrap();
    let stamps_at_edit = vault.doc.sync.stamps.clone();
    crate::usecases::sync::read::sync_manifest(&mut vault, None).unwrap();
    assert_ne!(
        vault.doc.sync.stamps, stamps_at_edit,
        "the sync call stamped the pending edit"
    );
    assert_eq!(tap.calls(), 1, "a sync call announces nothing itself");
    assert!(flush(&mut vault).is_some());
    assert_eq!(tap.calls(), 2);
    assert_eq!(tap.about(&id), 2, "the second edit announced once more");
    assert!(flush(&mut vault).is_none());
    assert_eq!(tap.calls(), 2);
}

#[test]
fn deleting_a_record_leaves_a_tombstone_and_trimming_a_log_does_not() {
    let (_dir, mut vault) = open_vault();
    vault.set_write_behind(Some(Arc::new(Wakes::default())));

    // A record: delete it, and its stamps go and a tombstone appears.
    let id = crate::usecases::household::contacts::create_contact(&mut vault, contact("Gone"))
        .unwrap()
        .id;
    flush(&mut vault);
    assert!(vault.doc.sync.stamps.keys().any(|key| key.contains(&id)));
    crate::usecases::household::contacts::delete_contact(&mut vault, &id).unwrap();
    flush(&mut vault);
    assert!(
        vault
            .doc
            .sync
            .tombstones
            .keys()
            .any(|key| key.contains(&id)),
        "a deleted record leaves a tombstone"
    );
    assert!(!vault.doc.sync.stamps.keys().any(|key| key.contains(&id)));

    // A capped log: the oldest records fall off, with no tombstone.
    let first: Vec<NoticeRecord> = (0..MAX_NOTICE_LOG)
        .map(|index| notice(&format!("log:{index:04}")))
        .collect();
    crate::usecases::planning::notifications::log_notices(&mut vault, first).unwrap();
    flush(&mut vault);
    let tombstones_before = vault.doc.sync.tombstones.clone();
    let more: Vec<NoticeRecord> = (MAX_NOTICE_LOG..MAX_NOTICE_LOG + 5)
        .map(|index| notice(&format!("log:{index:04}")))
        .collect();
    crate::usecases::planning::notifications::log_notices(&mut vault, more).unwrap();
    assert_eq!(vault.doc.notification_log.len(), MAX_NOTICE_LOG);
    assert!(
        vault
            .doc
            .notification_log
            .iter()
            .all(|record| record.key != "log:0000"),
        "the oldest record was trimmed"
    );
    flush(&mut vault);
    assert_eq!(
        vault.doc.sync.tombstones, tombstones_before,
        "trimming a log leaves no tombstone"
    );
    assert!(
        !vault
            .doc
            .sync
            .stamps
            .contains_key("notification_log:log:0000"),
        "the trimmed record's stamps are dropped"
    );
}
