//! A persistence that lives in memory: no Argon2 runs and no file is created, so a test
//! that only needs a document is fast.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use mwt_base::Result;
use mwt_vault::kernel::persistence::{Flushed, Persistence, SealedWrite};

/// A store that keeps every snapshot it is given and writes no file. Like the
/// file store it drops a job older than the newest written.
#[derive(Default)]
pub struct InMemoryPersistence {
    writes: AtomicUsize,
    snapshots: Mutex<Vec<Vec<u8>>>,
}

impl InMemoryPersistence {
    /// How many snapshots were written.
    pub fn write_count(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }

    /// The encoded snapshots, oldest first.
    pub fn snapshots(&self) -> Vec<Vec<u8>> {
        self.snapshots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Persistence for InMemoryPersistence {
    fn write(&self, job: SealedWrite) -> Result<Flushed> {
        let mut newest = job.gate.lock().unwrap_or_else(PoisonError::into_inner);
        let mut sealed = None;
        if job.revision > *newest {
            let bytes = job.plaintext.to_vec();
            self.snapshots
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(bytes.clone());
            self.writes.fetch_add(1, Ordering::SeqCst);
            *newest = job.revision;
            sealed = Some(bytes);
        }
        Ok(Flushed {
            path: job.path,
            revision: job.revision,
            sealed,
        })
    }

    fn writes(&self) -> bool {
        true
    }

    fn has_mirror(&self) -> bool {
        false
    }
}
