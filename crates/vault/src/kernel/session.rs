//! The write state of an open vault: what is unsaved, which revision it is,
//! and how a change is told to the writer. It moved here from `service/saving.rs`;
//! the secrets and the file lock stay in `UnlockedVault`.

use std::sync::{Arc, Mutex};

use super::events::{ChangeSink, Committed, DomainEvent, EventBus, KeysChanged, Subscription};
use super::persistence::{FileStore, Mirrored, Persistence, SandboxStore};

/// Portfolio base until the user picks another one.
pub const DEFAULT_BASE_CURRENCY: &str = "usd";

/// Told that the vault has unsaved changes (the app's writer thread).
pub trait SaveHook: Send + Sync {
    fn dirty(&self);
}

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
pub struct Write {
    /// Changes not on disk yet.
    dirty: bool,
    /// Bumped by every change; a job snapshots it.
    revision: u64,
    /// The newest revision on disk, shared with the jobs in flight.
    gate: Arc<Mutex<u64>>,
    /// Batched mode: wake the writer instead of writing.
    /// It is a subscriber of the bus: it wakes on `Committed`.
    hook: Option<Subscription>,
    /// The sync tap, as a subscriber of `KeysChanged`.
    sink: Option<Subscription>,
    /// Where the sealed snapshots go (the plain store, or it with a mirror).
    store: Arc<dyn Persistence>,
    /// The store without a mirror, to go back to.
    plain: Arc<dyn Persistence>,
    /// Subscribers of the events a committed transaction emits.
    bus: EventBus,
}

impl Default for Write {
    fn default() -> Self {
        Self::with_store(Arc::new(FileStore))
    }
}

impl Write {
    /// A session whose writes go nowhere: a sandbox copy.
    pub fn sandboxed() -> Self {
        Self::with_store(Arc::new(SandboxStore))
    }

    /// A session whose snapshots go to `store` (the test kit's in-memory one); called by the facade's `UnlockedVault`.
    pub fn with_store(store: Arc<dyn Persistence>) -> Self {
        Self {
            dirty: false,
            revision: 0,
            gate: Arc::default(),
            hook: None,
            sink: None,
            store: store.clone(),
            plain: store,
            bus: EventBus::new(),
        }
    }

    /// The vault's own bus.
    pub fn bus(&self) -> &EventBus {
        &self.bus
    }

    /// Publish the events of a finished transaction: only a touched one that
    /// returned `Ok` calls this; an untouched one drops them. Called by the facade's
    /// `UnlockedVault` commit (`service/saving.rs`).
    pub fn deliver(&self, touched: bool, events: Vec<DomainEvent>) {
        if !touched {
            return;
        }
        for event in &events {
            self.bus.publish(event);
        }
    }

    /// The store a flush job writes through.
    pub fn store(&self) -> Arc<dyn Persistence> {
        self.store.clone()
    }

    /// Copy every later write to `mirror`; `None` goes back to the plain store.
    pub fn set_mirror(&mut self, mirror: Option<Arc<dyn crate::vault_io::VaultMirror>>) {
        self.store = match mirror {
            Some(target) => Arc::new(Mirrored::new(self.plain.clone(), target)),
            None => self.plain.clone(),
        };
    }

    /// Whether writes are copied to an external document.
    pub fn has_mirror(&self) -> bool {
        self.store.has_mirror()
    }

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

    /// Tell the bus a marked change waits for the writer; the hook, if one is
    /// installed, wakes on it.
    pub fn announce_commit(&self) {
        self.bus.publish(
            &Committed {
                revision: self.revision,
            }
            .into(),
        );
    }

    /// Tell the bus the keys one save stamped.
    pub fn announce_changes(&self, keys: Vec<String>) {
        self.bus.publish(&KeysChanged { keys }.into());
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

    /// Replace the hook: it wakes once per `Committed` event (`None` stops).
    pub fn set_hook(&mut self, hook: Option<Arc<dyn SaveHook>>) {
        if let Some(old) = self.hook.take() {
            old.unsubscribe();
        }
        self.hook = hook.map(|target| {
            self.bus.subscribe(move |event| {
                if matches!(event, DomainEvent::Committed(_)) {
                    target.dirty();
                }
            })
        });
    }

    /// Replace the change sink: it gets the keys of every `KeysChanged`
    /// event (`None` stops).
    pub fn set_change_sink<S: ChangeSink + ?Sized + 'static>(&mut self, sink: Option<Arc<S>>) {
        if let Some(old) = self.sink.take() {
            old.unsubscribe();
        }
        self.sink = sink.map(|target| self.bus.subscribe_changes(target));
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

    struct Nowhere;

    impl crate::vault_io::VaultMirror for Nowhere {
        fn push(&self, _sealed: &[u8]) {}
    }

    #[test]
    fn has_mirror_follows_the_store() {
        let mut session = Write::default();
        assert!(!session.has_mirror());
        session.set_mirror(Some(Arc::new(Nowhere)));
        assert!(session.has_mirror());
    }

    #[test]
    fn set_mirror_none_goes_back_to_the_plain_store() {
        let mut session = Write::default();
        session.set_mirror(Some(Arc::new(Nowhere)));
        session.set_mirror(None);
        assert!(!session.has_mirror());
        assert!(session.store().writes());
    }

    #[test]
    fn mark_bumps_and_says_write_now_without_a_hook() {
        let mut session = Write::default();
        assert_eq!(session.mark(), Commit::WriteNow);
        assert!(session.is_dirty());
        assert_eq!(session.revision(), 1);
    }

    #[test]
    fn finish_of_an_older_revision_stays_dirty() {
        let mut session = Write::default();
        session.touch();
        session.touch();
        session.finish(1);
        assert!(session.is_dirty());
        session.finish(2);
        assert!(!session.is_dirty());
    }
}
