//! One transaction over the open document: the closure reads and changes it
//! through a `Tx`, and the vault commits only if the closure touched it.

use crate::kernel::collection::Collection;
use crate::kernel::record::Record;
use crate::model::VaultDoc;

/// A borrowed document plus the "touched" mark that decides whether the
/// commit writes. Reading never marks; mutable access and `touch` do.
pub struct Tx<'a> {
    doc: &'a mut VaultDoc,
    touched: bool,
    /// Sync keys of records deleted on purpose from a capped log; the commit
    /// hands them to `sync_forget_log_record`.
    forgotten_logs: Vec<String>,
}

impl<'a> Tx<'a> {
    pub(crate) fn new(doc: &'a mut VaultDoc) -> Self {
        Self {
            doc,
            touched: false,
            forgotten_logs: Vec::new(),
        }
    }

    /// Read the document; does not mark the transaction touched.
    pub fn doc(&self) -> &VaultDoc {
        self.doc
    }

    /// Change the document; marks the transaction touched.
    pub fn doc_mut(&mut self) -> &mut VaultDoc {
        self.touched = true;
        self.doc
    }

    /// Mark the transaction touched without changing anything here.
    pub fn touch(&mut self) {
        self.touched = true;
    }

    /// Mark the transaction touched when `changed` is true.
    pub fn touch_if(&mut self, changed: bool) {
        self.touched |= changed;
    }

    /// The records of one kind, read and changed through [`Collection`].
    pub fn collection<T: Record>(&mut self) -> Collection<'_, 'a, T> {
        Collection::new(self)
    }

    /// Remember the sync key of a record deleted from a capped log.
    pub(crate) fn forget_log_record(&mut self, key: String) {
        self.forgotten_logs.push(key);
    }

    /// The keys to hand to `sync_forget_log_record` at the commit.
    pub(crate) fn take_forgotten_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.forgotten_logs)
    }

    pub(crate) fn is_touched(&self) -> bool {
        self.touched
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_mutable_access_and_touch_mark_the_transaction() {
        let mut doc = VaultDoc::default();
        let mut tx = Tx::new(&mut doc);
        let _ = tx.doc();
        assert!(!tx.is_touched());
        tx.touch_if(false);
        assert!(!tx.is_touched());
        let _ = tx.doc_mut();
        assert!(tx.is_touched());
        let mut other = VaultDoc::default();
        let mut tx = Tx::new(&mut other);
        tx.touch();
        assert!(tx.is_touched());
    }
}
