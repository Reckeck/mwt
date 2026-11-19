//! The scenarios of the bulk area: several operations deleted, restored,
//! recategorized or moved at once. Each call is all or nothing: the items
//! are booked one after another on the open document (each is checked against
//! the ones before it), and the call ends in one commit; an error puts the
//! document back as it was and writes nothing.

use crate::error::{Error, Result};
use crate::model::transaction::Transaction;
use crate::money::bulk::{
    DeletedTransaction, TxCategories, TxStore, tx_index, with_categories, with_store,
};
use crate::service::UnlockedVault;
use crate::usecases::household::property_links::{check_property_link, set_property_link};

use super::stores::rebase_and_commit;
use super::transactions::prepare_transaction;

/// Run `change` on the document; on an error nothing of it stays, on success
/// the holdings are rebuilt and the document is written once.
fn all_or_nothing<T>(
    vault: &mut UnlockedVault,
    change: impl FnOnce(&mut UnlockedVault) -> Result<T>,
) -> Result<T> {
    let backup = vault.doc.clone();
    match change(vault) {
        Ok(out) => {
            rebase_and_commit(vault)?;
            Ok(out)
        }
        Err(err) => {
            vault.doc = backup;
            vault.recompute_holdings();
            Err(err)
        }
    }
}

/// Replace the operation at `index` by `tx` through the open transaction.
fn replace_at(vault: &mut UnlockedVault, index: usize, tx: Transaction) -> Result<()> {
    vault.run(|run| {
        run.doc_mut().transactions[index] = tx;
        Ok(())
    })?;
    Ok(())
}

/// Delete every operation of `ids`; an unknown id deletes none.
pub fn delete_transactions(
    vault: &mut UnlockedVault,
    ids: &[String],
) -> Result<Vec<DeletedTransaction>> {
    if ids.is_empty() {
        return Err(Error::InvalidInput("nothing to delete".into()));
    }
    all_or_nothing(vault, |vault| {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let index = tx_index(&vault.doc, id)?;
            let property_id = vault.property_of_tx(id).map(str::to_string);
            vault.cascade_shared_delete(id);
            let (transaction, _, _) =
                vault.run(|run| Ok(run.doc_mut().transactions.remove(index)))?;
            set_property_link(vault, id, None);
            out.push(DeletedTransaction {
                transaction,
                property_id,
            });
        }
        Ok(out)
    })
}

/// Put deleted operations back as they were: same id, origin and property.
pub fn restore_transactions(
    vault: &mut UnlockedVault,
    items: Vec<DeletedTransaction>,
) -> Result<()> {
    all_or_nothing(vault, |vault| {
        for item in items {
            let id = item.transaction.meta().id.clone();
            if tx_index(&vault.doc, &id).is_ok() {
                return Err(Error::InvalidInput(format!(
                    "transaction {id} already exists"
                )));
            }
            check_property_link(vault, item.property_id.as_deref())?;
            vault.run(|run| {
                run.doc_mut().transactions.push(item.transaction);
                Ok(())
            })?;
            set_property_link(vault, &id, item.property_id.as_deref());
        }
        vault.restore_removed_with_payment();
        Ok(())
    })
}

/// Replace the categories of each operation; returns the ones they had.
pub fn set_transaction_categories(
    vault: &mut UnlockedVault,
    changes: Vec<TxCategories>,
) -> Result<Vec<TxCategories>> {
    all_or_nothing(vault, |vault| {
        let mut before = Vec::with_capacity(changes.len());
        for change in changes {
            let index = tx_index(&vault.doc, &change.id)?;
            let (mut tx, had) = with_categories(&vault.doc.transactions[index], change);
            before.push(had);
            prepare_transaction(vault, &mut tx)?;
            replace_at(vault, index, tx)?;
        }
        Ok(before)
    })
}

/// Book payments and top-ups on another wallet; returns the wallets they had.
/// Two-sided kinds (transfer, exchange) are refused: which side would move?
pub fn move_transactions(vault: &mut UnlockedVault, changes: Vec<TxStore>) -> Result<Vec<TxStore>> {
    all_or_nothing(vault, |vault| {
        let mut before = Vec::with_capacity(changes.len());
        for change in changes {
            let index = tx_index(&vault.doc, &change.id)?;
            let (mut tx, had) = with_store(&vault.doc.transactions[index], change)?;
            before.push(had);
            prepare_transaction(vault, &mut tx)?;
            replace_at(vault, index, tx)?;
        }
        Ok(before)
    })
}
