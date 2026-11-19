//! The pure rules of the transactions area: what a new or edited operation
//! gets filled in with before it is checked and stored. Free functions over a
//! read context or a document; the scenarios that write live in
//! `usecases::money::transactions`.

use crate::kernel::books::Books;
use crate::model::store::StoreKind;
use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::VaultDoc;
use crate::money::rates::fx_at;
use crate::money::search::main_amount;

/// Fill what the user does not type: the purchase price of property
/// (its unit price is in the base currency at the time of buying).
pub fn complete_transaction(books: &Books, tx: &mut Transaction) {
    let Transaction::Topup(inner) = tx else {
        return;
    };
    let Some(unit_price) = inner.details.unit_price else {
        return;
    };
    let on_property = crate::money::tree::find_store(books.doc(), &inner.store_id)
        .is_some_and(|store| store.kind == StoreKind::Property);
    if !inner.price.is_empty() || !on_property {
        return;
    }
    let (base, _) = books.base_and_fx();
    inner.price.price_base = Some(unit_price);
    inner.price.price_base_currency = Some(base.to_string());
    inner.price.price_usd = fx_at(books, base, &inner.meta.timestamp)
        .filter(|rate| !rate.is_zero())
        .map(|rate| unit_price / rate);
}

/// An edit of one operation that names only what it changes (F4): the new
/// content of the operation, and the links it carries. A link the patch does
/// not name keeps its stored value; naming `Some(None)` clears it.
#[derive(Debug, Clone, PartialEq)]
pub struct TransactionPatch {
    /// The new content. Its own id, origin, flow and bill are not read: the
    /// fields below say what happens to those.
    pub tx: Transaction,
    /// The utility bill the operation settles.
    pub utility_item_id: Option<Option<String>>,
    /// The chain of operations it belongs to.
    pub flow_id: Option<Option<String>>,
    /// Where the record came from (the source link).
    pub origin: Option<TxOrigin>,
}

impl TransactionPatch {
    /// A patch that changes the content and names no link.
    pub fn of(tx: Transaction) -> Self {
        Self {
            tx,
            utility_item_id: None,
            flow_id: None,
            origin: None,
        }
    }
}

/// Give a new operation an id of its own when it has none or the id is taken.
pub fn ensure_fresh_id(doc: &VaultDoc, tx: &mut Transaction) {
    let id = tx.meta().id.trim();
    if id.is_empty() || doc.transactions.iter().any(|known| known.meta().id == id) {
        tx.meta_mut().id = uuid::Uuid::new_v4().to_string();
    }
}

/// The edit of `stored` as the form sent it: the id and the origin are kept
/// (an edit changes what happened, not where the record came from), and so are
/// the «keep» mark and the utility bill the operation settles (the form has no
/// such fields). A new amount for an operation booked approximately is the
/// real one.
pub fn edited(stored: &Transaction, id: &str, mut tx: Transaction) -> Transaction {
    let mut origin = stored.meta().origin.clone();
    if let TxOrigin::Workflow { estimated, .. } = &mut origin
        && *estimated
        && main_amount(stored) != main_amount(&tx)
    {
        *estimated = false;
    }
    let meta = tx.meta_mut();
    meta.id = id.to_string();
    meta.origin = origin;
    meta.keep = stored.meta().keep;
    meta.utility_item_id = stored.meta().utility_item_id.clone();
    tx
}
