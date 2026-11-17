//! The records of one kind inside a transaction: read, insert, update and
//! remove by `id`. Every mutator marks the transaction touched; `get` and
//! `iter` do not. A removal needs no extra step for a plain collection (the
//! change tracker writes the tombstone at the next save); a deletion from a
//! capped log is told to the commit, because the tracker would read it as a
//! trim.

use crate::error::{Error, Result};
use crate::kernel::record::Record;
use crate::kernel::tx::Tx;
use crate::sync::registry::{self, Rule};

/// A view of the `T` records of the document held by a [`Tx`].
pub struct Collection<'tx, 'doc, T: Record> {
    tx: &'tx mut Tx<'doc>,
    kind: std::marker::PhantomData<T>,
}

impl<'tx, 'doc, T: Record> Collection<'tx, 'doc, T> {
    pub(crate) fn new(tx: &'tx mut Tx<'doc>) -> Self {
        Self {
            tx,
            kind: std::marker::PhantomData,
        }
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.iter().find(|item| item.id() == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        T::slot_ref(self.tx.doc()).iter()
    }

    /// Add `item`; an id that is already there is refused and nothing changes.
    pub fn insert(&mut self, item: T) -> Result<()> {
        if self.get(item.id()).is_some() {
            return Err(Error::AlreadyExists(format!(
                "{} {}",
                T::COLLECTION,
                item.id()
            )));
        }
        T::slot(self.tx.doc_mut()).push(item);
        Ok(())
    }

    /// Edit the record with this id in place.
    pub fn update<R>(&mut self, id: &str, edit: impl FnOnce(&mut T) -> R) -> Result<R> {
        if self.get(id).is_none() {
            return Err(Self::not_found(id));
        }
        let item = T::slot(self.tx.doc_mut())
            .iter_mut()
            .find(|item| item.id() == id)
            .ok_or_else(|| Self::not_found(id))?;
        Ok(edit(item))
    }

    /// Take the record with this id out and return it.
    pub fn remove(&mut self, id: &str) -> Result<T> {
        let Some(index) = self.iter().position(|item| item.id() == id) else {
            return Err(Self::not_found(id));
        };
        let removed = T::slot(self.tx.doc_mut()).remove(index);
        if is_capped_log(T::COLLECTION) {
            self.tx
                .forget_log_record(format!("{}:{}", T::COLLECTION, id));
        }
        Ok(removed)
    }

    fn not_found(id: &str) -> Error {
        Error::NotFound(format!("{} {}", T::COLLECTION, id))
    }
}

/// Whether the sync registry reads a missing entry of this collection as a
/// trim, so a deliberate delete must be announced by hand.
fn is_capped_log(collection: &str) -> bool {
    registry::find(collection).is_some_and(|found| matches!(found.rule, Rule::Log { .. }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::VaultDoc;
    use crate::model::contact::Contact;
    use crate::model::vault::ImportRun;
    use crate::service::{FlushJob, SaveHook, UnlockedVault};

    struct NoWake;

    impl SaveHook for NoWake {
        fn dirty(&self) {}
    }

    fn contact(id: &str) -> Contact {
        Contact {
            id: id.into(),
            name: "Pilot".into(),
            ..Contact::default()
        }
    }

    fn run(id: &str) -> ImportRun {
        ImportRun {
            id: id.into(),
            ..ImportRun::default()
        }
    }

    fn batched_vault() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
        vault.set_device("aaaaaaaa-collection");
        vault.set_write_behind(Some(Arc::new(NoWake)));
        (dir, vault)
    }

    fn flush(vault: &mut UnlockedVault) {
        let job: Option<FlushJob> = vault.prepare_flush().unwrap();
        if let Some(job) = job {
            let done = job.write().unwrap();
            vault.finish_flush(&done);
        }
    }

    fn has_key(keys: impl IntoIterator<Item = impl AsRef<str>>, id: &str) -> bool {
        keys.into_iter().any(|key| key.as_ref().contains(id))
    }

    #[test]
    fn insert_update_remove_roundtrip_on_a_pilot_kind() {
        let mut doc = VaultDoc::default();
        let mut tx = Tx::new(&mut doc);
        let mut contacts = tx.collection::<Contact>();
        contacts.insert(contact("c1")).unwrap();
        assert_eq!(contacts.get("c1").unwrap().name, "Pilot");
        assert_eq!(contacts.iter().count(), 1);
        let seen = contacts
            .update("c1", |item| {
                std::mem::replace(&mut item.name, "Renamed".into())
            })
            .unwrap();
        assert_eq!(seen, "Pilot");
        assert_eq!(contacts.get("c1").unwrap().name, "Renamed");
        let removed = contacts.remove("c1").unwrap();
        assert_eq!(removed.name, "Renamed");
        assert!(contacts.get("c1").is_none());
        assert!(doc.contacts.is_empty());
    }

    #[test]
    fn a_duplicate_id_is_refused() {
        let mut doc = VaultDoc::default();
        let mut tx = Tx::new(&mut doc);
        let mut contacts = tx.collection::<Contact>();
        contacts.insert(contact("c1")).unwrap();
        let err = contacts.insert(contact("c1")).unwrap_err();
        assert!(matches!(err, Error::AlreadyExists(_)), "{err:?}");
        assert_eq!(contacts.iter().count(), 1);
    }

    #[test]
    fn removing_a_missing_id_is_not_found() {
        let mut doc = VaultDoc::default();
        let mut tx = Tx::new(&mut doc);
        let mut contacts = tx.collection::<Contact>();
        let err = contacts.remove("nope").unwrap_err();
        assert!(matches!(err, Error::NotFound(_)), "{err:?}");
        let err = contacts.update("nope", |_| ()).unwrap_err();
        assert!(matches!(err, Error::NotFound(_)), "{err:?}");
    }

    #[test]
    fn only_the_mutators_mark_the_transaction_touched() {
        let mut doc = VaultDoc::default();
        doc.contacts.push(contact("c1"));
        let mut tx = Tx::new(&mut doc);
        {
            let contacts = tx.collection::<Contact>();
            let _ = contacts.get("c1");
            let _ = contacts.iter().count();
        }
        assert!(!tx.is_touched());
        let _ = tx.collection::<Contact>().insert(contact("c1"));
        assert!(!tx.is_touched(), "a refused insert changes nothing");
        let _ = tx.collection::<Contact>().remove("nope");
        assert!(!tx.is_touched(), "a failed remove changes nothing");
        tx.collection::<Contact>().insert(contact("c2")).unwrap();
        assert!(tx.is_touched());

        for step in 0..2 {
            let mut other = VaultDoc::default();
            other.contacts.push(contact("c1"));
            let mut fresh = Tx::new(&mut other);
            if step == 0 {
                fresh.collection::<Contact>().update("c1", |_| ()).unwrap();
            } else {
                fresh.collection::<Contact>().remove("c1").unwrap();
            }
            assert!(fresh.is_touched(), "mutator {step}");
        }
    }

    #[test]
    fn a_record_removed_through_the_collection_gets_a_tombstone_in_the_next_save() {
        let (_dir, mut vault) = batched_vault();
        vault
            .transact(|tx| tx.collection::<Contact>().insert(contact("gone-1")))
            .unwrap();
        flush(&mut vault);
        assert!(has_key(vault.doc.sync.stamps.keys(), "gone-1"));
        vault
            .transact(|tx| tx.collection::<Contact>().remove("gone-1").map(|_| ()))
            .unwrap();
        flush(&mut vault);
        assert!(has_key(vault.doc.sync.tombstones.keys(), "gone-1"));
        assert!(!has_key(vault.doc.sync.stamps.keys(), "gone-1"));
    }

    #[test]
    fn a_log_record_removed_through_the_collection_is_announced_without_a_trim() {
        let (_dir, mut vault) = batched_vault();
        vault
            .transact(|tx| tx.collection::<ImportRun>().insert(run("run-1")))
            .unwrap();
        flush(&mut vault);
        vault
            .transact(|tx| tx.collection::<ImportRun>().remove("run-1").map(|_| ()))
            .unwrap();
        assert!(
            has_key(vault.doc.sync.tombstones.keys(), "run-1"),
            "the removal is a deletion, not a trim"
        );
        assert!(!has_key(vault.doc.sync.stamps.keys(), "run-1"));
        flush(&mut vault);
        assert!(has_key(vault.doc.sync.tombstones.keys(), "run-1"));
    }
}
