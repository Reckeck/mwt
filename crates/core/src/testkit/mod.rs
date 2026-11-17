//! Test kit: a vault that lives in memory. No Argon2 runs and no file is
//! created, so a test that only needs a document is fast. Compiled under
//! `cfg(any(test, feature = "testkit"))` only.

#[cfg(test)]
pub mod builders;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::error::Result;
use crate::kernel::persistence::{Persistence, SealedWrite};
use crate::model::VaultDoc;
use crate::service::{Flushed, UnlockedVault};

/// The money-blockers' clock: the one from `crate::time`, not a copy.
pub use crate::time::FixedClock;

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

impl UnlockedVault {
    /// A vault over `doc` kept in memory: plain key, detached lock, saves
    /// recorded by a fresh [`InMemoryPersistence`].
    pub fn in_memory(doc: VaultDoc) -> Self {
        Self::with_persistence(doc, Arc::new(InMemoryPersistence::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Amount;
    use crate::model::store::{AssetRef, StoreKind};
    use crate::model::transaction::*;
    use crate::time::Clock;

    fn topup(store_id: &str) -> Transaction {
        let mut meta = TxMeta::new("salary");
        meta.categories = vec!["salary".into()];
        Transaction::Topup(TopupTx {
            meta,
            store_id: store_id.into(),
            asset: AssetRef::fiat("USD", "US Dollar"),
            amount: Amount::parse("10").unwrap(),
            fee: None,
            from: None,
            details: KindDetails::default(),
            price: PriceStamp {
                price_usd: Some(Amount::parse("1").unwrap()),
                ..PriceStamp::default()
            },
        })
    }

    #[test]
    fn in_memory_vault_roundtrips_a_transaction() {
        let dir = tempfile::tempdir().unwrap();
        let before = std::fs::read_dir(dir.path()).unwrap().count();
        let store = Arc::new(InMemoryPersistence::default());
        let mut vault = UnlockedVault::with_persistence(VaultDoc::default(), store.clone());
        assert_eq!(store.write_count(), 0, "building the vault writes nothing");
        let wallet = vault
            .create_store("Bank", StoreKind::Bank, "#000", "", None)
            .unwrap();
        let writes = store.write_count();
        let added = vault.add_transaction(topup(&wallet.id)).unwrap();
        assert!(store.write_count() > writes, "the save went to the store");
        assert!(
            vault
                .doc
                .transactions
                .iter()
                .any(|tx| tx.meta().id == added.meta().id)
        );
        let last = store.snapshots().pop().unwrap();
        let doc = crate::storage::decode_versioned(&last, vault.fmt)
            .unwrap()
            .doc;
        assert!(
            doc.transactions
                .iter()
                .any(|tx| tx.meta().id == added.meta().id)
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), before);
        assert!(!vault.path.exists());
    }

    #[test]
    fn fixed_clock_is_the_money_blockers_one() {
        let kit: &dyn Clock = &FixedClock(7);
        let own: &dyn Clock = &crate::time::FixedClock(7);
        assert_eq!(kit.now_unix(), own.now_unix());
        let same: crate::time::FixedClock = FixedClock(1);
        assert_eq!(same.0, 1);
    }
}
