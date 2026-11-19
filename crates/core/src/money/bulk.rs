//! Several operations at once (Ops-1 · 4): delete, set the category, move to
//! another wallet. Each call is all or nothing and returns what an undo needs,
//! so the list offers "Undo" instead of asking "Are you sure?".

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::transaction::Transaction;
use crate::model::vault::VaultDoc;

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

/// Where the operation `id` stands in the document.
pub fn tx_index(doc: &VaultDoc, id: &str) -> Result<usize> {
    doc.transactions
        .iter()
        .position(|tx| tx.meta().id == id)
        .ok_or_else(|| Error::Missing("transaction".into()))
}

/// The operation with the categories of `change`, and the ones it had.
pub fn with_categories(tx: &Transaction, change: TxCategories) -> (Transaction, TxCategories) {
    let mut moved = tx.clone();
    let before = TxCategories {
        id: change.id,
        categories: moved.meta().categories.clone(),
    };
    moved.meta_mut().categories = change.categories;
    (moved, before)
}

/// The operation booked on the wallet of `change`, and the wallet it had.
/// Two-sided kinds (transfer, exchange) are refused: which side would move?
pub fn with_store(tx: &Transaction, change: TxStore) -> Result<(Transaction, TxStore)> {
    let mut moved = tx.clone();
    let store = match &mut moved {
        Transaction::Payment(inner) => &mut inner.store_id,
        Transaction::Topup(inner) => &mut inner.store_id,
        _ => {
            return Err(Error::InvalidInput(
                "only a payment or a top-up can move to another wallet".into(),
            ));
        }
    };
    let before = TxStore {
        id: change.id,
        store_id: store.clone(),
    };
    *store = change.store_id;
    Ok((moved, before))
}
