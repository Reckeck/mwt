//! Several operations at once (Ops-1 · 4): delete, set the category, move to
//! another wallet. Each call is all or nothing and returns what an undo needs,
//! so the list offers "Undo" instead of asking "Are you sure?".

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::transaction::Transaction;

use super::UnlockedVault;

/// A deleted operation and the property it was linked to: enough to put it back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct DeletedTransaction {
    pub transaction: Transaction,
    #[serde(default)]
    pub property_id: Option<String>,
}

/// The categories of one operation: the new ones on the way in, the old ones back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct TxCategories {
    pub id: String,
    pub categories: Vec<String>,
}

/// The wallet of a one-sided operation (payment, top-up).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct TxStore {
    pub id: String,
    pub store_id: String,
}

impl UnlockedVault {
    fn tx_index(&self, id: &str) -> Result<usize> {
        self.doc
            .transactions
            .iter()
            .position(|tx| tx.meta().id == id)
            .ok_or_else(|| Error::Missing("transaction".into()))
    }

    /// Run `change` on the document; on an error nothing of it stays.
    fn all_or_nothing<T>(&mut self, change: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let backup = self.doc.clone();
        match change(self) {
            Ok(out) => {
                self.recompute_holdings();
                self.persist()?;
                Ok(out)
            }
            Err(err) => {
                self.doc = backup;
                self.recompute_holdings();
                Err(err)
            }
        }
    }

    /// Delete every operation of `ids`; an unknown id deletes none.
    pub fn delete_transactions(&mut self, ids: &[String]) -> Result<Vec<DeletedTransaction>> {
        if ids.is_empty() {
            return Err(Error::InvalidInput("nothing to delete".into()));
        }
        self.all_or_nothing(|vault| {
            let mut out = Vec::with_capacity(ids.len());
            for id in ids {
                let index = vault.tx_index(id)?;
                let property_id = vault.property_of_tx(id).map(str::to_string);
                vault.cascade_shared_delete(id);
                let transaction = vault.doc.transactions.remove(index);
                vault.set_property_link(id, None);
                out.push(DeletedTransaction {
                    transaction,
                    property_id,
                });
            }
            Ok(out)
        })
    }

    /// Put deleted operations back as they were: same id, origin and property.
    pub fn restore_transactions(&mut self, items: Vec<DeletedTransaction>) -> Result<()> {
        self.all_or_nothing(|vault| {
            for item in items {
                let id = item.transaction.meta().id.clone();
                if vault.tx_index(&id).is_ok() {
                    return Err(Error::InvalidInput(format!(
                        "transaction {id} already exists"
                    )));
                }
                vault.check_property_link(item.property_id.as_deref())?;
                vault.doc.transactions.push(item.transaction);
                vault.set_property_link(&id, item.property_id.as_deref());
            }
            vault.restore_removed_with_payment();
            Ok(())
        })
    }

    /// Replace the categories of each operation; returns the ones they had.
    pub fn set_transaction_categories(
        &mut self,
        changes: Vec<TxCategories>,
    ) -> Result<Vec<TxCategories>> {
        self.all_or_nothing(|vault| {
            let mut before = Vec::with_capacity(changes.len());
            for change in changes {
                let index = vault.tx_index(&change.id)?;
                let mut tx = vault.doc.transactions[index].clone();
                before.push(TxCategories {
                    id: change.id,
                    categories: tx.meta().categories.clone(),
                });
                tx.meta_mut().categories = change.categories;
                vault.prepare_transaction(&mut tx)?;
                vault.doc.transactions[index] = tx;
            }
            Ok(before)
        })
    }

    /// Book payments and top-ups on another wallet; returns the wallets they had.
    /// Two-sided kinds (transfer, exchange) are refused: which side would move?
    pub fn move_transactions(&mut self, changes: Vec<TxStore>) -> Result<Vec<TxStore>> {
        self.all_or_nothing(|vault| {
            let mut before = Vec::with_capacity(changes.len());
            for change in changes {
                let index = vault.tx_index(&change.id)?;
                let mut tx = vault.doc.transactions[index].clone();
                let store = match &mut tx {
                    Transaction::Payment(inner) => &mut inner.store_id,
                    Transaction::Topup(inner) => &mut inner.store_id,
                    _ => {
                        return Err(Error::InvalidInput(
                            "only a payment or a top-up can move to another wallet".into(),
                        ));
                    }
                };
                before.push(TxStore {
                    id: change.id,
                    store_id: store.clone(),
                });
                *store = change.store_id;
                vault.prepare_transaction(&mut tx)?;
                vault.doc.transactions[index] = tx;
            }
            Ok(before)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;

    use super::*;

    fn two_payments() -> (tempfile::TempDir, UnlockedVault, String, Vec<String>) {
        let (dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        vault
            .add_transaction(income(&card, "100", "salary"))
            .unwrap();
        let ids = ["food", "taxi"]
            .iter()
            .map(|comment| {
                vault
                    .add_transaction(payment(&card, "10", comment))
                    .unwrap()
                    .meta()
                    .id
                    .clone()
            })
            .collect();
        (dir, vault, card, ids)
    }

    #[test]
    fn delete_and_restore_round_trip() {
        let (_dir, mut vault, card, ids) = two_payments();
        let deleted = vault.delete_transactions(&ids).unwrap();
        assert_eq!(deleted.len(), 2);
        assert_eq!(holding_of(&vault, &card, &usd()), amount("100"));
        vault.restore_transactions(deleted).unwrap();
        assert_eq!(holding_of(&vault, &card, &usd()), amount("80"));
        let restored: Vec<_> = vault
            .doc
            .transactions
            .iter()
            .map(|tx| tx.meta().id.clone())
            .collect();
        assert!(ids.iter().all(|id| restored.contains(id)));
    }

    #[test]
    fn an_unknown_id_deletes_nothing() {
        let (_dir, mut vault, _card, mut ids) = two_payments();
        ids.push("nope".into());
        assert!(vault.delete_transactions(&ids).is_err());
        assert_eq!(vault.doc.transactions.len(), 3);
    }

    #[test]
    fn categories_are_set_and_given_back() {
        let (_dir, mut vault, _card, ids) = two_payments();
        let changes = ids
            .iter()
            .map(|id| TxCategories {
                id: id.clone(),
                categories: vec!["transport".into()],
            })
            .collect();
        let before = vault.set_transaction_categories(changes).unwrap();
        assert!(before.iter().all(|entry| entry.categories == ["groceries"]));
        let now = vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == ids[0])
            .unwrap();
        assert_eq!(now.meta().categories, ["transport"]);
        vault.set_transaction_categories(before).unwrap();
        let back = vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == ids[0])
            .unwrap();
        assert_eq!(back.meta().categories, ["groceries"]);
    }

    #[test]
    fn payments_move_but_transfers_do_not() {
        let (_dir, mut vault, card, ids) = two_payments();
        let other = store(&mut vault, "Other", StoreKind::Bank, None);
        let before = vault
            .move_transactions(vec![TxStore {
                id: ids[0].clone(),
                store_id: other.clone(),
            }])
            .unwrap();
        assert_eq!(before[0].store_id, card);
        assert_eq!(holding_of(&vault, &other, &usd()), amount("-10"));

        let moved = vault
            .add_transaction(transfer_between(
                crate::model::transaction::AccountRef::Store(card.clone()),
                crate::model::transaction::AccountRef::Store(other.clone()),
                usd(),
                "5",
            ))
            .unwrap();
        let refused = vault.move_transactions(vec![
            TxStore {
                id: ids[1].clone(),
                store_id: other.clone(),
            },
            TxStore {
                id: moved.meta().id.clone(),
                store_id: card.clone(),
            },
        ]);
        assert!(refused.is_err());
        // All or nothing: the payment stayed where it was.
        let second = vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == ids[1])
            .unwrap();
        assert_eq!(second.primary_store_id(), Some(card.as_str()));
    }
}
