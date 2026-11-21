//! Test kit: a vault that lives in memory. No Argon2 runs and no file is
//! created, so a test that only needs a document is fast. Compiled under
//! `cfg(any(test, feature = "testkit"))` only.
//!
//! The model-only part (properties, builders, `InMemoryPersistence`, `FixedClock`) is the
//! dev-only crate `mwt-testkit`, re-exported here so the old `testkit::*` paths keep
//! working; what needs `UnlockedVault` stays in this module.

use std::sync::Arc;

use crate::model::VaultDoc;
use crate::service::UnlockedVault;

pub use mwt_testkit::*;

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
        vault.path = dir.path().join("vault.bin");
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
        let doc = crate::storage::decode(&last, vault.fmt).unwrap();
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

    #[test]
    fn built_records_sit_in_an_in_memory_vault() {
        let wallet = builders::store("Bank", StoreKind::Bank);
        let income = builders::topup(&wallet.id, "25");
        let mut doc = VaultDoc::default();
        doc.stores.push(wallet);
        doc.transactions.push(income);
        doc.goals.push(builders::goal("Trip", "500"));
        let vault = UnlockedVault::in_memory(doc);
        assert_eq!(vault.doc.stores.len(), 1);
        assert_eq!(vault.doc.transactions.len(), 1);
        assert_eq!(vault.doc.goals[0].name, "Trip");
    }
}
