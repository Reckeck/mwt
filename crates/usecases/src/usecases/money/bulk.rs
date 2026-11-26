//! The scenarios of the bulk area: several operations deleted, restored,
//! recategorized or moved at once. Each call is all or nothing: the items
//! are booked one after another on the open document (each is checked against
//! the ones before it), and the call ends in one commit; an error puts the
//! document back as it was and writes nothing.

use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::model::transaction::Transaction;
use crate::money::bulk::{
    DeletedTransaction, TxCategories, TxStore, tx_index, with_categories, with_store,
};
use crate::service::Session;
use crate::usecases::household::property_links::{check_property_link, set_property_link};

use super::stores::rebase_and_commit;
use super::transactions::{prepare_transaction, refuse_forged_income_ref, refuse_if_linked};
use crate::money::distribution::linked_to_income;
use crate::usecases::planning::distribution::{
    apply_distribution_unsaved, drop_distribution_of, rows_of,
};

/// Run `change` on the document; on an error nothing of it stays, on success
/// the holdings are rebuilt and the document is written once.
fn all_or_nothing<T>(
    vault: &mut Session,
    change: impl FnOnce(&mut Session) -> Result<T>,
) -> Result<T> {
    let backup = vault.doc.clone();
    match change(vault) {
        Ok(out) => {
            rebase_and_commit(vault)?;
            Ok(out)
        }
        Err(err) => {
            vault.doc = backup;
            crate::usecases::money::portfolio::recompute_holdings(vault);
            Err(err)
        }
    }
}

/// Replace the operation at `index` by `tx` through the open transaction.
fn replace_at(vault: &mut Session, index: usize, tx: Transaction) -> Result<()> {
    vault.run(|run| {
        run.doc_mut().transactions[index] = tx;
        Ok(())
    })?;
    Ok(())
}

/// Delete every operation of `ids`; an unknown id deletes none.
pub fn delete_transactions(vault: &mut Session, ids: &[String]) -> Result<Vec<DeletedTransaction>> {
    if ids.is_empty() {
        return Err(Error::InvalidInput("nothing to delete".into()));
    }
    all_or_nothing(vault, |vault| {
        // A linked effect goes with its income; on its own it is refused.
        let mut with_income = Vec::new();
        for id in ids {
            let index = tx_index(&vault.doc, id)?;
            let stored = &vault.doc.transactions[index];
            match &stored.meta().income_ref {
                Some(income) if ids.contains(income) => with_income.push(id),
                _ => refuse_if_linked(stored)?,
            }
        }
        // The stored incomes with their distribution, kept for the undo.
        let mut stored_before = HashMap::with_capacity(ids.len());
        for id in ids {
            let index = tx_index(&vault.doc, id)?;
            stored_before.insert(id, vault.doc.transactions[index].clone());
        }
        for id in ids {
            drop_distribution_of(vault, id)?;
        }
        let mut out = Vec::with_capacity(ids.len());
        for id in ids.iter().filter(|id| !with_income.contains(id)) {
            let index = tx_index(&vault.doc, id)?;
            let property_id = crate::household::property_links::property_of_tx(&vault.doc, id)
                .map(str::to_string);
            crate::usecases::household::shared_events::cascade_shared_delete(vault, id);
            vault.run(|run| Ok(run.doc_mut().transactions.remove(index)))?;
            let transaction = stored_before
                .remove(id)
                .ok_or_else(|| Error::Missing(format!("transaction {id}")))?;
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
pub fn restore_transactions(vault: &mut Session, items: Vec<DeletedTransaction>) -> Result<()> {
    all_or_nothing(vault, |vault| {
        for item in items {
            refuse_forged_income_ref(&item.transaction)?;
            let id = item.transaction.meta().id.clone();
            if tx_index(&vault.doc, &id).is_ok() {
                return Err(Error::InvalidInput(format!(
                    "transaction {id} already exists"
                )));
            }
            check_property_link(vault, item.property_id.as_deref())?;
            let mut transaction = item.transaction;
            // A distribution goes back by re-applying its rows: the effects are booked anew.
            let distribution = match &mut transaction {
                Transaction::Topup(topup) => topup.distribution.take(),
                _ => None,
            };
            vault.run(|run| {
                run.doc_mut().transactions.push(transaction);
                Ok(())
            })?;
            set_property_link(vault, &id, item.property_id.as_deref());
            if let Some(parts) = distribution {
                apply_distribution_unsaved(vault, &id, &rows_of(parts))?;
            }
        }
        crate::usecases::sync::repair::restore_removed_with_payment(vault);
        Ok(())
    })
}

/// Replace the categories of each operation; returns the ones they had.
pub fn set_transaction_categories(
    vault: &mut Session,
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
pub fn move_transactions(vault: &mut Session, changes: Vec<TxStore>) -> Result<Vec<TxStore>> {
    all_or_nothing(vault, |vault| {
        let mut before = Vec::with_capacity(changes.len());
        for change in changes {
            let index = tx_index(&vault.doc, &change.id)?;
            if matches!(&vault.doc.transactions[index], Transaction::Topup(topup) if topup.distribution.is_some())
            {
                return Err(linked_to_income("an income with a distribution"));
            }
            let (mut tx, had) = with_store(&vault.doc.transactions[index], change)?;
            before.push(had);
            prepare_transaction(vault, &mut tx)?;
            replace_at(vault, index, tx)?;
        }
        Ok(before)
    })
}
