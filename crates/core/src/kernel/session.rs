//! The write state of an open vault: what is unsaved, which revision it is,
//! and how a change is told to the writer. It moved here from `service/saving.rs`;
//! the secrets and the file lock stay in `UnlockedVault`.

use std::sync::{Arc, Mutex};

use crate::service::SaveHook;

/// What the caller does after a change is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commit {
    /// A hook is installed: wake the writer, which batches the write.
    Wake,
    /// No hook: write the file at once.
    WriteNow,
    /// Nothing is written (a sandbox).
    Nothing,
}

/// Batching state of an open vault.
#[derive(Default)]
pub struct Session {
    /// Changes not on disk yet.
    dirty: bool,
    /// Bumped by every change; a job snapshots it.
    revision: u64,
    /// The newest revision on disk, shared with the jobs in flight.
    gate: Arc<Mutex<u64>>,
    /// Batched mode: wake the writer instead of writing.
    hook: Option<Arc<dyn SaveHook>>,
}

impl Session {
    /// Bump the revision and set dirty, with no word on how it gets written.
    pub fn touch(&mut self) {
        self.revision += 1;
        self.dirty = true;
    }

    /// Bump the revision, set dirty, and say what happens next.
    pub fn mark(&mut self) -> Commit {
        self.touch();
        if self.hook.is_some() {
            Commit::Wake
        } else {
            Commit::WriteNow
        }
    }

    /// Wake the writer, if one is installed.
    pub fn wake(&self) {
        if let Some(hook) = &self.hook {
            hook.dirty();
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The newest-written-revision gate, shared with the jobs in flight.
    pub fn gate(&self) -> Arc<Mutex<u64>> {
        self.gate.clone()
    }

    pub fn set_hook(&mut self, hook: Option<Arc<dyn SaveHook>>) {
        self.hook = hook;
    }

    /// Clean, unless the document changed after `written`.
    pub fn finish(&mut self, written: u64) {
        if written == self.revision {
            self.dirty = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_bumps_and_says_write_now_without_a_hook() {
        let mut session = Session::default();
        assert_eq!(session.mark(), Commit::WriteNow);
        assert!(session.is_dirty());
        assert_eq!(session.revision(), 1);
    }

    #[test]
    fn finish_of_an_older_revision_stays_dirty() {
        let mut session = Session::default();
        session.touch();
        session.touch();
        session.finish(1);
        assert!(session.is_dirty());
        session.finish(2);
        assert!(!session.is_dirty());
    }
}
