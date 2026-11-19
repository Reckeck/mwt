//! The scenarios of the transactions area: an operation added, saved in a
//! group, edited or deleted. What an operation is filled in with comes from
//! `money::transactions`; here the rules of other areas are called, the
//! document is changed through one `run`, and the holdings are rebuilt and
//! committed once at the end (`rebase_and_commit`).

use crate::error::{Error, Result};
use crate::model::store::StoreScope;
use crate::model::transaction::Transaction;
use crate::model::vault::{iso_to_millis, now_iso};
use crate::money::transactions::{complete_transaction, edited, ensure_fresh_id};
use crate::service::UnlockedVault;
use crate::usecases::household::property_links::{check_property_link, set_property_link};
use crate::usecases::household::shared_events::cascade_shared_delete;

use super::stores::rebase_and_commit;

/// Transaction history, newest first, limited to a scope.
pub fn transactions(vault: &UnlockedVault, scope: &StoreScope) -> Vec<Transaction> {
    let ids = (!scope.store_ids.is_empty()).then(|| vault.resolve_scope(scope));
    let mut out: Vec<Transaction> = match ids {
        Some(ids) => vault.scoped_txs(&ids),
        None => vault.doc.transactions.clone(),
    };
    out.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
    out
}

/// Fill what the user does not type, at the moment a write happens.
pub fn prepare_transaction(vault: &UnlockedVault, tx: &mut Transaction) -> Result<()> {
    prepare_transaction_at(vault, tx, iso_to_millis(&now_iso()))
}

/// Everything a write does before storing: categories normalized and
/// auto-filled, property prices completed, unpriced sides stamped, every
/// rule checked. The future-date rule (V11) is measured from `now_ms` — the
/// scheduler's clock for automation runs.
pub fn prepare_transaction_at(
    vault: &UnlockedVault,
    tx: &mut Transaction,
    now_ms: i64,
) -> Result<()> {
    UnlockedVault::normalize_categories(tx);
    vault.apply_categorization_rules(tx);
    complete_transaction(&vault.books(), tx);
    vault.complete_adjustment(tx)?;
    UnlockedVault::stamp_prices(&vault.rate_book(), tx);
    vault.validate_transaction_at(tx, now_ms)
}

/// Validate and append a transaction of any variant (plan §4.5, §8.1):
/// categories are normalized and auto-filled, then every rule is checked.
pub fn add_transaction(vault: &mut UnlockedVault, tx: Transaction) -> Result<Transaction> {
    add_transaction_for(vault, tx, None)
}

/// [`add_transaction`], linked to a property (plan G4).
pub fn add_transaction_for(
    vault: &mut UnlockedVault,
    mut tx: Transaction,
    property_id: Option<&str>,
) -> Result<Transaction> {
    check_property_link(vault, property_id)?;
    prepare_transaction(vault, &mut tx)?;
    ensure_fresh_id(&vault.doc, &mut tx);
    vault.run(|run| {
        run.doc_mut().transactions.push(tx.clone());
        Ok(())
    })?;
    set_property_link(vault, &tx.meta().id, property_id);
    rebase_and_commit(vault)?;
    Ok(tx)
}

/// Save several operations as one movement of money (an exchange split
/// between recipients, a P2P deal): all join one chain — `flow_id`, or a
/// new one — and either all are saved or none is.
pub fn add_linked_transactions(
    vault: &mut UnlockedVault,
    txs: Vec<Transaction>,
    flow_id: Option<&str>,
) -> Result<Vec<Transaction>> {
    if txs.is_empty() {
        return Err(Error::InvalidInput("nothing to save".into()));
    }
    let flow = flow_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let backup = vault.doc.clone();
    match book_linked(vault, txs, &flow) {
        Ok(saved) => {
            rebase_and_commit(vault)?;
            Ok(saved)
        }
        Err(err) => {
            vault.doc = backup;
            vault.recompute_holdings();
            Err(err)
        }
    }
}

fn book_linked(
    vault: &mut UnlockedVault,
    txs: Vec<Transaction>,
    flow: &str,
) -> Result<Vec<Transaction>> {
    let mut saved = Vec::with_capacity(txs.len());
    for mut tx in txs {
        tx.meta_mut().flow_id = Some(flow.to_string());
        tx.meta_mut().id = uuid::Uuid::new_v4().to_string();
        prepare_transaction(vault, &mut tx)?;
        vault.run(|run| {
            run.doc_mut().transactions.push(tx.clone());
            Ok(())
        })?;
        // The next one sees this one's balances, as if saved in turn.
        vault.recompute_holdings();
        saved.push(tx);
    }
    Ok(saved)
}

/// Replace a transaction entirely. Its id and origin are kept: an edit
/// changes what happened, not where the record came from.
pub fn update_transaction(
    vault: &mut UnlockedVault,
    id: &str,
    tx: Transaction,
) -> Result<Transaction> {
    let property = vault.property_of_tx(id).map(str::to_string);
    edit(vault, id, tx, property.as_deref(), true)
}

/// [`update_transaction`] for the shared event that owns the operation: the
/// event has changed its own side already.
pub(crate) fn update_transaction_unsynced(
    vault: &mut UnlockedVault,
    id: &str,
    tx: Transaction,
) -> Result<Transaction> {
    let property = vault.property_of_tx(id).map(str::to_string);
    edit(vault, id, tx, property.as_deref(), false)
}

/// [`update_transaction`] that also sets the operation's property; `None`
/// unlinks it.
pub fn update_transaction_for(
    vault: &mut UnlockedVault,
    id: &str,
    tx: Transaction,
    property_id: Option<&str>,
) -> Result<Transaction> {
    edit(vault, id, tx, property_id, true)
}

/// `follow_events`: an operation a shared event owns drags the event's
/// purchase or settlement along (or is refused).
fn edit(
    vault: &mut UnlockedVault,
    id: &str,
    tx: Transaction,
    property_id: Option<&str>,
    follow_events: bool,
) -> Result<Transaction> {
    check_property_link(vault, property_id)?;
    let index = position_of(&vault.doc.transactions, id)?;
    let mut tx = edited(&vault.doc.transactions[index], id, tx);
    prepare_transaction(vault, &mut tx)?;
    let event_edit = if follow_events {
        vault.plan_shared_tx_edit(id, &tx)?
    } else {
        None
    };
    vault.run(|run| {
        run.doc_mut().transactions[index] = tx.clone();
        Ok(())
    })?;
    vault.apply_shared_edit(event_edit);
    set_property_link(vault, id, property_id);
    rebase_and_commit(vault)?;
    Ok(tx)
}

fn position_of(transactions: &[Transaction], id: &str) -> Result<usize> {
    transactions
        .iter()
        .position(|known| known.meta().id == id)
        .ok_or_else(|| Error::Missing("transaction".into()))
}

pub fn delete_transaction(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    let index = position_of(&vault.doc.transactions, id)?;
    vault.run(|run| {
        run.doc_mut().transactions.remove(index);
        Ok(())
    })?;
    cascade_shared_delete(vault, id);
    set_property_link(vault, id, None);
    rebase_and_commit(vault)
}
