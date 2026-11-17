//! The port behind every save: where a sealed snapshot goes. A vault on disk
//! writes through [`FileStore`]; a sandbox copy writes through [`SandboxStore`],
//! which does no I/O at all, so the writing code never asks "is this a sandbox".

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use zeroize::Zeroizing;

use crate::crypto::SealKey;
use crate::error::Result;
use crate::service::Flushed;
use crate::storage::{SnapshotFormat, VaultFile};

/// One encoded snapshot on its way to a store, still to be sealed.
pub struct SealedWrite {
    pub path: PathBuf,
    pub fmt: SnapshotFormat,
    pub plaintext: Zeroizing<Vec<u8>>,
    pub key: Arc<SealKey>,
    pub revision: u64,
    /// The newest revision written, shared with the jobs in flight.
    pub gate: Arc<Mutex<u64>>,
}

/// Where a snapshot is written.
pub trait Persistence: Send + Sync {
    /// Write the job. `Flushed::sealed` holds the bytes that went out, or
    /// `None` when nothing was written (a stale job, or no store behind).
    fn write(&self, job: SealedWrite) -> Result<Flushed>;
    /// False for the null store.
    fn writes(&self) -> bool;
    /// Whether saves are copied to an external document.
    fn has_mirror(&self) -> bool;
}

/// The vault file: seal and write atomically, never backwards.
#[derive(Debug, Default, Clone, Copy)]
pub struct FileStore;

impl Persistence for FileStore {
    fn write(&self, job: SealedWrite) -> Result<Flushed> {
        let mut newest = job.gate.lock().unwrap_or_else(PoisonError::into_inner);
        let mut sealed = None;
        if job.revision > *newest {
            sealed = Some(VaultFile::seal_and_write(
                &job.path,
                &job.key,
                &job.plaintext,
                job.fmt,
            )?);
            *newest = job.revision;
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

/// Null Object for a sandbox copy: no file, no gate, no copy.
#[derive(Debug, Default, Clone, Copy)]
pub struct SandboxStore;

impl Persistence for SandboxStore {
    fn write(&self, job: SealedWrite) -> Result<Flushed> {
        Ok(Flushed {
            path: job.path,
            revision: job.revision,
            sealed: None,
        })
    }

    fn writes(&self) -> bool {
        false
    }

    fn has_mirror(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(path: PathBuf, revision: u64) -> SealedWrite {
        SealedWrite {
            path,
            fmt: SnapshotFormat::for_profile(),
            plaintext: Zeroizing::new(b"{}".to_vec()),
            key: Arc::new(SealKey::plain()),
            revision,
            gate: Arc::new(Mutex::new(0)),
        }
    }

    #[test]
    fn a_sandbox_store_never_touches_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sandbox.wallet");
        let store = SandboxStore;
        let done = store.write(job(path.clone(), 1)).unwrap();
        assert!(!path.exists());
        assert_eq!(done.sealed, None);
        assert!(!store.writes());
        assert!(!store.has_mirror());
    }

    #[test]
    fn a_file_store_writes_and_drops_a_stale_job() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wallet");
        let store = FileStore;
        let fresh = job(path.clone(), 2);
        let gate = fresh.gate.clone();
        assert!(store.write(fresh).unwrap().sealed.is_some());
        assert!(path.exists());
        let mut stale = job(path, 1);
        stale.gate = gate;
        assert_eq!(store.write(stale).unwrap().sealed, None);
        assert!(store.writes());
    }
}
