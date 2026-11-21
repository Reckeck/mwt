//! The port behind every save: where a sealed snapshot goes. A vault on disk
//! writes through [`FileStore`]; a sandbox session holds [`SandboxStore`], which
//! does no I/O at all. The sandbox gate in the service write path stays until a
//! later wave.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use zeroize::Zeroizing;

use crate::storage::{SnapshotFormat, VaultFile};
use crate::vault_io::VaultMirror;
use mwt_base::Result;
use mwt_crypto::SealKey;

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

/// A job that went to disk (or was dropped as older than what is there).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flushed {
    pub path: PathBuf,
    pub revision: u64,
    /// The bytes that went out; `None` when nothing was written.
    pub sealed: Option<Vec<u8>>,
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

/// Decorator: after the inner store wrote, the sealed bytes go to the mirror.
/// A job the inner store dropped (stale, or no store behind) is not pushed.
pub struct Mirrored {
    inner: Arc<dyn Persistence>,
    mirror: Arc<dyn VaultMirror>,
}

impl Mirrored {
    pub fn new(inner: Arc<dyn Persistence>, mirror: Arc<dyn VaultMirror>) -> Self {
        Self { inner, mirror }
    }
}

impl Persistence for Mirrored {
    fn write(&self, job: SealedWrite) -> Result<Flushed> {
        let done = self.inner.write(job)?;
        if let Some(sealed) = &done.sealed {
            self.mirror.push(sealed);
        }
        Ok(done)
    }

    fn writes(&self) -> bool {
        self.inner.writes()
    }

    fn has_mirror(&self) -> bool {
        true
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

    #[derive(Default)]
    struct Recording(Mutex<Vec<Vec<u8>>>);

    impl VaultMirror for Recording {
        fn push(&self, sealed: &[u8]) {
            self.0.lock().unwrap().push(sealed.to_vec());
        }
    }

    #[test]
    fn a_mirrored_store_pushes_after_the_inner_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.wallet");
        let recording = Arc::new(Recording::default());
        let store = Mirrored::new(Arc::new(FileStore), recording.clone());
        let fresh = job(path.clone(), 2);
        let gate = fresh.gate.clone();
        let done = store.write(fresh).unwrap();
        assert!(store.has_mirror());
        assert!(store.writes());
        let pushed = recording.0.lock().unwrap().clone();
        assert_eq!(pushed, vec![done.sealed.unwrap()]);
        assert_eq!(pushed[0], std::fs::read(&path).unwrap());
        // A job the gate drops is not pushed.
        let mut stale = job(path, 1);
        stale.gate = gate;
        assert_eq!(store.write(stale).unwrap().sealed, None);
        assert_eq!(recording.0.lock().unwrap().len(), 1);
    }
}
