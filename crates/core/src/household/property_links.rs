//! Operations that belong to a property (plan G4): rent received for a flat,
//! fuel and insurance for a car. The link lives beside the operation
//! (`VaultDoc::tx_properties`), and the property's summary sums it up.
//!
//! Pure part of the area. The payments of a utility item live here too: the
//! year of a property counts its bills, and the utilities read them through
//! the same function, so the two modules call each other through no vault.

use crate::Amount;
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::model::planning::UtilityItem;
use crate::model::store::{Store, StoreKind};
use crate::model::transaction::{PaymentTx, Transaction};
use crate::model::vault::{TxProperty, VaultDoc};
use crate::time::secs_or_now;

/// The summary's look back: a year.
const YEAR_SECS: i64 = 365 * 86_400;

/// What a property brought and cost over the last year, in the base currency.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropertyYear {
    pub rent: Amount,
    pub expenses: Amount,
    /// What its value lost over the year; zero when it grew.
    pub wear: Amount,
}

/// Every operation's property, for the operation form.
pub fn tx_property_links(doc: &VaultDoc) -> Vec<TxProperty> {
    doc.tx_properties.clone()
}

pub fn property_of_tx<'doc>(doc: &'doc VaultDoc, tx_id: &str) -> Option<&'doc str> {
    doc.tx_properties
        .iter()
        .find(|link| link.tx_id == tx_id)
        .map(|link| link.property_id.as_str())
}

/// A link may point only at a live property: `store` is the live store the
/// operation is about to be linked to.
pub fn require_property(store: &Store) -> Result<()> {
    if store.kind != StoreKind::Property {
        return Err(Error::InvalidInput(
            "an operation is linked only to property".into(),
        ));
    }
    Ok(())
}

/// Link the operation to a property, or unlink it with `None`. Writes nothing
/// to disk: the caller's own transaction does.
pub fn replace_link(doc: &mut VaultDoc, tx_id: &str, property_id: Option<&str>) {
    doc.tx_properties.retain(|link| link.tx_id != tx_id);
    if let Some(id) = property_id {
        doc.tx_properties.push(TxProperty {
            tx_id: tx_id.to_string(),
            property_id: id.to_string(),
        });
    }
}

/// Payments made for an item: linked to it, or — from before the link —
/// with its name as the comment, its category and its store.
pub fn item_payments<'doc>(
    doc: &'doc VaultDoc,
    item: &'doc UtilityItem,
) -> impl Iterator<Item = &'doc PaymentTx> + 'doc {
    doc.transactions.iter().filter_map(move |tx| {
        let Transaction::Payment(payment) = tx else {
            return None;
        };
        let linked = match payment.meta.utility_item_id.as_deref() {
            Some(id) => id == item.id,
            None => {
                payment.meta.comment.trim() == item.name
                    && payment.meta.categories.contains(&item.category)
                    && item
                        .store_id
                        .as_ref()
                        .is_none_or(|id| *id == payment.store_id)
            }
        };
        linked.then_some(payment)
    })
}

/// The rent, the expenses and the loss of value of a property over the
/// year before `now`. Expenses are the operations linked to it, its
/// utility bills and the payments into the loans it secures.
pub fn property_year(books: &Books<'_>, store_id: &str, now: i64) -> PropertyYear {
    let doc = books.doc();
    let (base, _) = books.base_and_fx();
    let book = books.rate_book();
    let from = now - YEAR_SECS;
    let in_year = |tx: &Transaction| {
        let secs = secs_or_now(&tx.meta().timestamp);
        (from..=now).contains(&secs)
    };
    let to_base = |tx: &Transaction, store: Option<&str>, sign: i8| {
        let secs = secs_or_now(&tx.meta().timestamp);
        tx.legs()
            .iter()
            .filter(|leg| store.is_none_or(|id| leg.store_id == id))
            .filter(|leg| match sign {
                1 => leg.amount.is_positive(),
                _ => leg.amount.is_negative(),
            })
            .fold(Amount::ZERO, |sum, leg| {
                let amount = leg.amount.abs();
                let value = book
                    .convert(amount, &leg.asset.symbol, base, secs)
                    .or_else(|| {
                        let usd = book.price_at(&leg.asset.key(), secs)?;
                        book.convert(amount * usd, "usd", base, secs)
                    })
                    .unwrap_or(Amount::ZERO);
                sum + value
            })
    };
    let mut out = PropertyYear::default();
    let mut counted: Vec<&str> = Vec::new();
    for tx in &doc.transactions {
        if property_of_tx(doc, &tx.meta().id) != Some(store_id) || !in_year(tx) {
            continue;
        }
        match tx {
            // Rent comes to a money account; buying the thing itself is no rent.
            Transaction::Topup(topup) if topup.store_id != store_id => {
                out.rent += to_base(tx, None, 1);
            }
            Transaction::Topup(_) => {}
            _ => out.expenses += to_base(tx, None, -1),
        }
        counted.push(tx.meta().id.as_str());
    }
    let items = doc
        .utility_items
        .iter()
        .filter(|item| item.property_id == store_id);
    for item in items {
        for payment in item_payments(doc, item) {
            let tx = Transaction::Payment(payment.clone());
            if in_year(&tx) && !counted.contains(&payment.meta.id.as_str()) {
                out.expenses += to_base(&tx, None, -1);
            }
        }
    }
    let loans: Vec<&str> = doc
        .stores
        .iter()
        .filter(|loan| loan.secured_by.as_deref() == Some(store_id))
        .map(|loan| loan.id.as_str())
        .collect();
    for tx in &doc.transactions {
        if !in_year(tx) || counted.contains(&tx.meta().id.as_str()) {
            continue;
        }
        for loan in &loans {
            out.expenses += to_base(tx, Some(loan), 1);
        }
    }
    out.wear = property_wear(books, store_id, from, now);
    out
}

/// How much the property's value fell between `from` and `now`, in the
/// base currency; zero without a value curve or when it grew.
pub fn property_wear(books: &Books<'_>, store_id: &str, from: i64, now: i64) -> Amount {
    let (base, _) = books.base_and_fx();
    let book = books.rate_book();
    let mut lost = Amount::ZERO;
    for holding in books
        .doc()
        .holdings
        .iter()
        .filter(|holding| holding.store_id == store_id)
    {
        let Some(curve) = book.curves().get(&holding.asset.key()) else {
            continue;
        };
        let (Some(then), Some(today)) = (curve.value_at(from), curve.value_at(now)) else {
            continue;
        };
        let drop = (then - today) * holding.quantity;
        if drop.is_positive() {
            lost += book
                .convert(drop, curve.currency(), base, now)
                .unwrap_or(Amount::ZERO);
        }
    }
    lost
}
