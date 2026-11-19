//! Rule of the move into the module (spec E11) that works on the document alone:
//! archiving the wallets the move retires. Which wallets those are, and why, is
//! decided in `usecases::cryptotrack::transition`, where the reasoning is
//! written down.

use std::collections::BTreeSet;

use crate::model::vault::VaultDoc;

/// Set `Store::archived` on the wallets with these ids. Deletes no record and
/// saves nothing; the caller recounts the holdings.
pub fn archive_wallets(doc: &mut VaultDoc, store_ids: &BTreeSet<&str>) {
    for store in &mut doc.stores {
        if store_ids.contains(store.id.as_str()) {
            store.archived = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::{Store, StoreKind};

    #[test]
    fn archiving_marks_only_the_named_wallets_and_deletes_nothing() {
        let mut doc = VaultDoc::default();
        let named = Store::new("Old cold", StoreKind::Crypto, "#000");
        let kept = Store::new("New cold", StoreKind::Crypto, "#000");
        let (named_id, kept_id) = (named.id.clone(), kept.id.clone());
        doc.stores = vec![named, kept];

        archive_wallets(&mut doc, &BTreeSet::from([named_id.as_str()]));

        assert_eq!(doc.stores.len(), 2);
        let flag = |id: &str| {
            doc.stores
                .iter()
                .find(|store| store.id == id)
                .unwrap()
                .archived
        };
        assert!(flag(&named_id));
        assert!(!flag(&kept_id));
    }
}
