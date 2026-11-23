//! Saving the open vault: right away, or batched (write-behind).
//!
//! Every mutation ends in [`UnlockedVault::persist`]. Without a [`SaveHook`]
//! (tests, tools) that writes the file at once, as it always did. The app
//! installs a hook: `persist` then only marks the document dirty and wakes the
//! app's writer, which batches the changes of a burst into one write
//! ([`UnlockedVault::prepare_flush`] under the vault lock, [`FlushJob::write`]
//! outside it, [`UnlockedVault::finish_flush`] back under it). Whatever must be
//! on disk *now* (a new password, a backup copy, closing the vault) calls
//! [`UnlockedVault::persist_now`].
//!
//! Writes never go backwards: every job carries the revision it snapshotted,
//! and a shared gate remembers the newest revision written, so a slow older
//! job that finishes after a newer one is dropped instead of overwriting it.

use std::path::Path;
use std::sync::Arc;

use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::storage::encode;
use crate::usecases::sync::peer_sync;

use crate::kernel::events::DomainEvent;
pub use crate::kernel::persistence::Flushed;
use crate::kernel::persistence::{Persistence, SealedWrite};
use crate::kernel::session::Commit;
pub use crate::kernel::session::SaveHook;
use crate::kernel::tx::Tx;

use super::UnlockedVault;

/// One snapshot of the document on its way to disk, sealed off the vault lock.
pub struct FlushJob {
    write: SealedWrite,
    store: Arc<dyn Persistence>,
}

impl std::fmt::Debug for FlushJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlushJob")
            .field("revision", &self.write.revision)
            .finish_non_exhaustive()
    }
}

impl FlushJob {
    /// Seal and write. Needs no vault lock; writes are serialised by the gate.
    pub fn write(self) -> Result<Flushed> {
        self.store.write(self.write)
    }
}

impl UnlockedVault {
    /// Run `body` over the document and commit if it touched it: at once, or
    /// — once the app installed a hook — mark it dirty for the next batched
    /// write. An untouched transaction writes nothing and bumps nothing; on
    /// `Err` nothing is committed (and nothing rolled back). A sandbox never
    /// writes.
    pub fn transact<R>(&mut self, body: impl FnOnce(&mut Tx<'_>) -> Result<R>) -> Result<R> {
        let (out, touched, events) = self.run(body)?;
        if touched && !self.sandbox {
            let commit = self.session.mark();
            self.apply_commit(commit)?;
        }
        self.session.deliver(touched, events);
        Ok(out)
    }

    /// Do what a marked change asks. `Nothing` neither writes nor bumps.
    pub(crate) fn apply_commit(&mut self, commit: Commit) -> Result<()> {
        match commit {
            Commit::Wake => self.session.announce_commit(),
            Commit::WriteNow => self.write_now()?,
            Commit::Nothing => {}
        }
        Ok(())
    }

    /// Like [`Self::transact`], but a touched transaction is on disk when it
    /// returns, whatever is batched.
    pub fn transact_now<R>(&mut self, body: impl FnOnce(&mut Tx<'_>) -> Result<R>) -> Result<R> {
        let (out, touched, events) = self.run(body)?;
        if touched && !self.sandbox {
            self.write_now()?;
        }
        self.session.deliver(touched, events);
        Ok(out)
    }

    /// Run the closure over the document; report whether it touched it and
    /// the events it emitted (the caller delivers them once the commit
    /// succeeded; an `Err` drops them). The log records it deleted on purpose
    /// are handed to the sync layer here, so the next save writes their
    /// tombstones.
    pub(crate) fn run<R>(
        &mut self,
        body: impl FnOnce(&mut Tx<'_>) -> Result<R>,
    ) -> Result<(R, bool, Vec<DomainEvent>)> {
        let mut tx = Tx::new(&mut self.doc);
        let out = body(&mut tx)?;
        let touched = tx.is_touched();
        let events = tx.take_events();
        let forgotten = tx.take_forgotten_logs();
        for key in forgotten {
            peer_sync::sync_forget_log_record(self, &key)?;
        }
        Ok((out, touched, events))
    }

    /// Save the document: at once, or — once the app installed a hook — mark
    /// it dirty for the next batched write. A sandbox never writes.
    pub fn persist(&mut self) -> Result<()> {
        self.transact(|tx| {
            tx.touch();
            Ok(())
        })
    }

    /// Write the document now, whatever is batched: the file must be current
    /// before it is copied, re-keyed or closed.
    pub fn persist_now(&mut self) -> Result<()> {
        self.transact_now(|tx| {
            tx.touch();
            Ok(())
        })
    }

    /// Bump the revision and write the snapshot to disk.
    pub(crate) fn write_now(&mut self) -> Result<()> {
        self.session.touch();
        let Some(job) = self.prepare_flush()? else {
            return Ok(());
        };
        let done = job.write()?;
        self.finish_flush(&done);
        Ok(())
    }

    /// Write batched changes, if any, before something reads the file itself.
    pub fn flush_pending(&mut self) -> Result<()> {
        if self.session.is_dirty() {
            self.persist_now()
        } else {
            Ok(())
        }
    }

    /// Batched mode: from now on `persist` wakes `hook` instead of writing.
    pub fn set_write_behind(&mut self, hook: Option<Arc<dyn SaveHook>>) {
        if !self.sandbox {
            self.session.set_hook(hook);
        }
    }

    /// Whether changes are waiting for the writer.
    pub fn has_unsaved_changes(&self) -> bool {
        self.session.is_dirty()
    }

    /// Snapshot the dirty document for the writer (under the vault lock):
    /// stamps the changes for sync, encodes, and returns the job to seal off
    /// the lock. `None` when nothing is waiting.
    pub fn prepare_flush(&mut self) -> Result<Option<FlushJob>> {
        if self.sandbox || !self.session.is_dirty() {
            return Ok(None);
        }
        let key = self
            .key
            .clone()
            .ok_or_else(|| Error::Crypto("the vault has no key to save with".into()))?;
        peer_sync::track_changes(self)?;
        // Everything stamped since the last push, by this save or by a sync
        // call in between.
        let changed: Vec<String> = std::mem::take(&mut self.announce).into_iter().collect();
        self.announce_to_tap(changed);
        // Level 3: exchange keys go to the device file, and the document is
        // written without them (the in-memory index stays).
        self.flush_device_keys()?;
        let encoded = if self.keys_on_device() {
            let index = std::mem::take(&mut self.doc.exchanges);
            let encoded = encode(&self.doc, self.fmt);
            self.doc.exchanges = index;
            encoded
        } else {
            encode(&self.doc, self.fmt)
        };
        let plaintext = Zeroizing::new(encoded?);
        Ok(Some(FlushJob {
            write: SealedWrite {
                path: self.path.clone(),
                fmt: self.fmt,
                plaintext,
                key,
                revision: self.session.revision(),
                gate: self.session.gate(),
            },
            store: self.session.store(),
        }))
    }

    /// Tell the sync tap which keys changed; nothing when no tap is set or
    /// there are no keys. The one home of that rule.
    pub(super) fn announce_to_tap(&mut self, keys: Vec<String>) {
        if self.sync_tap.is_some() && !keys.is_empty() {
            self.session.announce_changes(keys);
        }
    }

    /// Back under the vault lock after [`FlushJob::write`]: clean unless the
    /// document changed meanwhile (then the next batch carries it).
    pub fn finish_flush(&mut self, done: &Flushed) {
        if done.path == self.path && done.revision == self.session.revision() {
            self.session.finish(done.revision);
        }
    }

    /// Whether `path` is this vault's file (a job may outlive a vault switch).
    pub fn is_file(&self, path: &Path) -> bool {
        self.path == path
    }
}

impl Drop for UnlockedVault {
    /// Closing (lock, switch, exit) must not lose batched changes. The app
    /// flushes before it gets here; this is the last line, so it says so.
    fn drop(&mut self) {
        if self.session.is_dirty()
            && !self.sandbox
            && let Err(err) = self.persist_now()
        {
            eprintln!("vault: unsaved changes lost on close: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::super::test_support::empty_vault;
    use super::*;
    use crate::crypto::parse_file;
    use crate::storage::VaultFile;

    /// Counts the wake-ups instead of running a writer thread.
    #[derive(Default)]
    struct Wakes(AtomicUsize);

    impl SaveHook for Wakes {
        fn dirty(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn batched() -> (tempfile::TempDir, UnlockedVault, Arc<Wakes>) {
        let (dir, mut vault) = empty_vault();
        let wakes = Arc::new(Wakes::default());
        vault.set_write_behind(Some(wakes.clone()));
        (dir, vault, wakes)
    }

    fn change(vault: &mut UnlockedVault, decimals: u32) {
        vault.doc.settings.display_decimals = decimals;
        vault.persist().unwrap();
    }

    fn decimals_on_disk(path: &Path) -> u32 {
        VaultFile::open(path, "pw")
            .unwrap()
            .0
            .settings
            .display_decimals
    }

    fn flush(vault: &mut UnlockedVault) -> Option<Flushed> {
        let job = vault.prepare_flush().unwrap()?;
        let done = job.write().unwrap();
        vault.finish_flush(&done);
        Some(done)
    }

    #[test]
    fn commit_nothing_neither_writes_nor_bumps_the_revision() {
        let (_dir, mut vault, wakes) = batched();
        let before = std::fs::read(&vault.path).unwrap();
        let revision = vault.session.revision();
        vault.apply_commit(Commit::Nothing).unwrap();
        assert_eq!(vault.session.revision(), revision);
        assert!(!vault.has_unsaved_changes());
        assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
        assert_eq!(std::fs::read(&vault.path).unwrap(), before);
    }

    #[test]
    fn set_write_behind_still_wakes_once_per_commit() {
        let (_dir, mut vault, wakes) = batched();
        change(&mut vault, 3);
        assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
        change(&mut vault, 4);
        assert_eq!(wakes.0.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_burst_of_changes_reaches_the_file_in_one_write() {
        let (_dir, mut vault, wakes) = batched();
        let before = std::fs::read(&vault.path).unwrap();
        for decimals in 0..50 {
            change(&mut vault, decimals % 8);
        }
        assert_eq!(wakes.0.load(Ordering::SeqCst), 50);
        assert_eq!(
            std::fs::read(&vault.path).unwrap(),
            before,
            "nothing written before the flush"
        );
        assert!(vault.has_unsaved_changes());
        assert!(flush(&mut vault).is_some());
        assert!(!vault.has_unsaved_changes());
        assert_eq!(decimals_on_disk(&vault.path), 49 % 8);
        assert!(flush(&mut vault).is_none(), "a clean vault writes nothing");
    }

    #[test]
    fn a_change_during_the_write_waits_for_the_next_batch() {
        let (_dir, mut vault, _) = batched();
        change(&mut vault, 3);
        let job = vault.prepare_flush().unwrap().unwrap();
        change(&mut vault, 5);
        let done = job.write().unwrap();
        vault.finish_flush(&done);
        assert!(vault.has_unsaved_changes());
        assert_eq!(decimals_on_disk(&vault.path), 3);
        flush(&mut vault);
        assert_eq!(decimals_on_disk(&vault.path), 5);
    }

    #[test]
    fn an_older_job_never_overwrites_a_newer_write() {
        let (_dir, mut vault, _) = batched();
        change(&mut vault, 2);
        let stale = vault.prepare_flush().unwrap().unwrap();
        change(&mut vault, 6);
        vault.persist_now().unwrap();
        stale.write().unwrap();
        assert_eq!(decimals_on_disk(&vault.path), 6);
        assert!(!vault.has_unsaved_changes());
    }

    #[test]
    fn a_failed_write_keeps_the_changes_for_the_next_try() {
        let (dir, mut vault, _) = batched();
        change(&mut vault, 4);
        let real = vault.path.clone();
        // A regular file where the directory should be: the write cannot succeed.
        let blocker = dir.path().join("not-a-dir");
        std::fs::write(&blocker, b"").unwrap();
        vault.path = blocker.join("test.wallet");
        let job = vault.prepare_flush().unwrap().unwrap();
        assert!(job.write().is_err());
        assert!(vault.has_unsaved_changes());
        vault.path = real;
        flush(&mut vault);
        assert_eq!(decimals_on_disk(&vault.path), 4);
    }

    #[test]
    fn closing_a_vault_writes_what_was_batched() {
        let (_dir, mut vault, _) = batched();
        change(&mut vault, 7);
        let path = vault.path.clone();
        drop(vault);
        assert_eq!(decimals_on_disk(&path), 7);
    }

    #[test]
    fn saves_reuse_the_unlock_key_instead_of_running_the_kdf() {
        let (_dir, mut vault) = empty_vault();
        vault.persist_now().unwrap();
        let first = std::fs::read(&vault.path).unwrap();
        change(&mut vault, 1);
        let second = std::fs::read(&vault.path).unwrap();
        let (one, _) = parse_file(&first).unwrap();
        let (two, _) = parse_file(&second).unwrap();
        // A fresh salt per save meant a fresh Argon2 run per save.
        assert_eq!(one.salt, two.salt);
        assert_ne!(one.nonce, two.nonce);
    }

    #[test]
    fn a_new_password_gets_a_new_salt_and_is_on_disk_at_once() {
        let (_dir, mut vault, _) = batched();
        let before = parse_file(&std::fs::read(&vault.path).unwrap())
            .unwrap()
            .0
            .salt;
        vault.change_password("pw", "new").unwrap();
        let bytes = std::fs::read(&vault.path).unwrap();
        assert_ne!(parse_file(&bytes).unwrap().0.salt, before);
        assert!(matches!(
            VaultFile::open(&vault.path, "pw"),
            Err(Error::Secret)
        ));
        assert!(VaultFile::open(&vault.path, "new").is_ok());
        // Later saves keep sealing under the new password.
        change(&mut vault, 2);
        flush(&mut vault);
        assert_eq!(
            VaultFile::open(&vault.path, "new")
                .unwrap()
                .0
                .settings
                .display_decimals,
            2
        );
    }

    #[test]
    fn a_sandbox_never_writes_even_when_batched() {
        let (_dir, vault, _) = batched();
        let before = std::fs::read(&vault.path).unwrap();
        let mut sandbox = vault.sandbox();
        sandbox.set_write_behind(Some(Arc::new(Wakes::default())));
        sandbox.doc.settings.display_decimals = 5;
        sandbox.persist().unwrap();
        assert!(sandbox.prepare_flush().unwrap().is_none());
        drop(sandbox);
        assert_eq!(std::fs::read(&vault.path).unwrap(), before);
    }

    #[test]
    fn a_settings_patch_that_changes_nothing_writes_nothing() {
        let (_dir, mut vault, wakes) = batched();
        let same = crate::ipc_contract::UpdateSettingsInput {
            display_decimals: Some(vault.doc.settings.display_decimals),
            ..Default::default()
        };
        vault.update_settings(same).unwrap();
        assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
        assert!(!vault.has_unsaved_changes());
        let other = crate::ipc_contract::UpdateSettingsInput {
            display_decimals: Some(vault.doc.settings.display_decimals + 1),
            ..Default::default()
        };
        vault.update_settings(other).unwrap();
        assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_read_only_transaction_does_not_write() {
        let (_dir, mut vault, wakes) = batched();
        let before = std::fs::read(&vault.path).unwrap();
        let revision = vault.session.revision();
        let decimals = vault.transact(|tx| Ok(tx.doc().settings.display_decimals));
        assert!(decimals.is_ok());
        assert_eq!(vault.session.revision(), revision);
        assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
        assert!(!vault.has_unsaved_changes());
        assert_eq!(std::fs::read(&vault.path).unwrap(), before);
    }

    #[test]
    fn an_error_in_the_closure_commits_nothing() {
        let (_dir, mut vault, wakes) = batched();
        let before = std::fs::read(&vault.path).unwrap();
        let result: Result<()> = vault.transact(|tx| {
            tx.doc_mut().settings.display_decimals = 3;
            Err(Error::Secret)
        });
        assert!(result.is_err());
        assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
        assert!(!vault.has_unsaved_changes());
        assert_eq!(std::fs::read(&vault.path).unwrap(), before);
    }

    fn count_events(vault: &UnlockedVault) -> Arc<AtomicUsize> {
        let count = Arc::new(AtomicUsize::new(0));
        let seen = count.clone();
        // The wake-up of a hooked commit is an event too; it is not the
        // transaction's own.
        vault.session.bus().subscribe(move |event| {
            if !matches!(event, DomainEvent::Committed(_)) {
                seen.fetch_add(1, Ordering::SeqCst);
            }
        });
        count
    }

    fn event() -> DomainEvent {
        crate::kernel::events::KeysChanged { keys: Vec::new() }.into()
    }

    /// What a subscriber of the transaction's own event could see the moment
    /// it was called: the newest revision announced as committed (the wake-up
    /// of a batched commit) and the decimals already on disk.
    #[derive(Default)]
    struct Seen {
        announced: Option<u64>,
        delivered_with: Option<(Option<u64>, u32)>,
    }

    fn watch_delivery(vault: &UnlockedVault) -> Arc<std::sync::Mutex<Seen>> {
        let seen = Arc::new(std::sync::Mutex::new(Seen::default()));
        let record = seen.clone();
        let path = vault.path.clone();
        vault.session.bus().subscribe(move |event| {
            let mut state = record.lock().unwrap();
            match event {
                DomainEvent::Committed(committed) => state.announced = Some(committed.revision),
                _ => state.delivered_with = Some((state.announced, decimals_on_disk(&path))),
            }
        });
        seen
    }

    #[test]
    fn a_committed_transaction_delivers_its_events_after_commit() {
        for now in [false, true] {
            let (_dir, mut vault, _) = batched();
            let count = count_events(&vault);
            let seen = watch_delivery(&vault);
            let before = vault.session.revision();
            let old_decimals = decimals_on_disk(&vault.path);
            let new_decimals = old_decimals + 1;
            let during = count.clone();
            let body = move |tx: &mut Tx<'_>| {
                tx.doc_mut().settings.display_decimals = new_decimals;
                tx.touch();
                tx.emit(event());
                assert_eq!(during.load(Ordering::SeqCst), 0);
                Ok(())
            };
            if now {
                vault.transact_now(body).unwrap();
            } else {
                vault.transact(body).unwrap();
            }
            assert_eq!(count.load(Ordering::SeqCst), 1);
            let (announced, on_disk) = seen.lock().unwrap().delivered_with.unwrap();
            if now {
                // The file was written before the subscriber ran.
                assert_eq!(on_disk, new_decimals);
            } else {
                // The revision was bumped and announced (dirty, writer woken)
                // before the subscriber ran; the file waits for the writer.
                assert_eq!(announced, Some(before + 1));
                assert_eq!(on_disk, old_decimals);
            }
        }
    }

    #[test]
    fn an_untouched_or_failed_transaction_drops_its_events() {
        let (_dir, mut vault, _) = batched();
        let count = count_events(&vault);
        vault
            .transact(|tx| {
                tx.emit(event());
                Ok(())
            })
            .unwrap();
        let failed: Result<()> = vault.transact(|tx| {
            tx.touch();
            tx.emit(event());
            Err(Error::Secret)
        });
        assert!(failed.is_err());
        let failed_now: Result<()> = vault.transact_now(|tx| {
            tx.touch();
            tx.emit(event());
            Err(Error::Secret)
        });
        assert!(failed_now.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
