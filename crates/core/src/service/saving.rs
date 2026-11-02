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

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use zeroize::Zeroizing;

use crate::crypto::SealKey;
use crate::error::{Error, Result};
use crate::storage::{SnapshotFormat, VaultFile, encode};

use super::UnlockedVault;

/// Told that the vault has unsaved changes (the app's writer thread).
pub trait SaveHook: Send + Sync {
    fn dirty(&self);
}

/// Batching state of an open vault.
#[derive(Default)]
pub(super) struct SaveState {
    /// Changes not on disk yet.
    dirty: bool,
    /// Bumped by every change; a job snapshots it.
    revision: u64,
    /// The newest revision on disk, shared with the jobs in flight.
    gate: Arc<Mutex<u64>>,
    /// Batched mode: wake the writer instead of writing.
    hook: Option<Arc<dyn SaveHook>>,
}

/// One snapshot of the document on its way to disk, sealed off the vault lock.
pub struct FlushJob {
    path: PathBuf,
    fmt: SnapshotFormat,
    plaintext: Zeroizing<Vec<u8>>,
    key: Arc<SealKey>,
    revision: u64,
    gate: Arc<Mutex<u64>>,
    mirror: Option<Arc<dyn crate::vault_io::VaultMirror>>,
}

impl std::fmt::Debug for FlushJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlushJob")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

/// A job that went to disk (or was dropped as older than what is there).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flushed {
    pub path: PathBuf,
    pub revision: u64,
}

impl FlushJob {
    /// Seal and write. Needs no vault lock; writes are serialised by the gate.
    pub fn write(self) -> Result<Flushed> {
        let mut newest = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        if self.revision > *newest {
            let sealed =
                VaultFile::seal_and_write(&self.path, &self.key, &self.plaintext, self.fmt)?;
            *newest = self.revision;
            if let Some(mirror) = &self.mirror {
                mirror.push(&sealed);
            }
        }
        Ok(Flushed {
            path: self.path,
            revision: self.revision,
        })
    }
}

impl UnlockedVault {
    /// Save the document: at once, or — once the app installed a hook — mark
    /// it dirty for the next batched write. A sandbox never writes.
    pub fn persist(&mut self) -> Result<()> {
        if self.sandbox {
            return Ok(());
        }
        self.save.revision += 1;
        self.save.dirty = true;
        if let Some(hook) = &self.save.hook {
            hook.dirty();
            return Ok(());
        }
        self.persist_now()
    }

    /// Write the document now, whatever is batched: the file must be current
    /// before it is copied, re-keyed or closed.
    pub fn persist_now(&mut self) -> Result<()> {
        if self.sandbox {
            return Ok(());
        }
        self.save.revision += 1;
        self.save.dirty = true;
        let Some(job) = self.prepare_flush()? else {
            return Ok(());
        };
        let done = job.write()?;
        self.finish_flush(&done);
        Ok(())
    }

    /// Write batched changes, if any, before something reads the file itself.
    pub fn flush_pending(&mut self) -> Result<()> {
        if self.save.dirty {
            self.persist_now()
        } else {
            Ok(())
        }
    }

    /// Batched mode: from now on `persist` wakes `hook` instead of writing.
    pub fn set_write_behind(&mut self, hook: Option<Arc<dyn SaveHook>>) {
        if !self.sandbox {
            self.save.hook = hook;
        }
    }

    /// Whether changes are waiting for the writer.
    pub fn has_unsaved_changes(&self) -> bool {
        self.save.dirty
    }

    /// Snapshot the dirty document for the writer (under the vault lock):
    /// stamps the changes for sync, encodes, and returns the job to seal off
    /// the lock. `None` when nothing is waiting.
    pub fn prepare_flush(&mut self) -> Result<Option<FlushJob>> {
        if self.sandbox || !self.save.dirty {
            return Ok(None);
        }
        let key = self
            .key
            .clone()
            .ok_or_else(|| Error::Crypto("the vault has no key to save with".into()))?;
        self.track_changes()?;
        // Everything stamped since the last push, by this save or by a sync
        // call in between.
        let changed: Vec<String> = std::mem::take(&mut self.announce).into_iter().collect();
        if !changed.is_empty()
            && let Some(tap) = &self.sync_tap
        {
            tap.changed(&changed);
        }
        let plaintext = Zeroizing::new(encode(&self.doc, self.fmt)?);
        Ok(Some(FlushJob {
            path: self.path.clone(),
            fmt: self.fmt,
            plaintext,
            key,
            revision: self.save.revision,
            gate: self.save.gate.clone(),
            mirror: self.mirror.clone(),
        }))
    }

    /// Back under the vault lock after [`FlushJob::write`]: clean unless the
    /// document changed meanwhile (then the next batch carries it).
    pub fn finish_flush(&mut self, done: &Flushed) {
        if done.path == self.path && done.revision == self.save.revision {
            self.save.dirty = false;
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
        if self.save.dirty
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
}
