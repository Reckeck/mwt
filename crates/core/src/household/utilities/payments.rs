//! What was paid for an item, by month. Which payments belong to an item is
//! decided in `household::property_links::item_payments`, the same function
//! the property's year counts its bills with.

use std::collections::BTreeMap;

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::model::planning::UtilityItem;
use crate::model::transaction::Transaction;
use crate::money::books::Books;
use crate::time::secs_or_now;

use crate::household::property_links::item_payments;

/// The item's payments by period, in its currency.
pub fn paid_by_month(
    books: &Books<'_>,
    item: &UtilityItem,
    cal: &Calendar,
) -> BTreeMap<String, Amount> {
    let book = books.rate_book();
    let mut out: BTreeMap<String, Amount> = BTreeMap::new();
    for payment in item_payments(books.doc(), item) {
        let secs = secs_or_now(&payment.meta.timestamp);
        let amount = if payment.asset.symbol.eq_ignore_ascii_case(&item.currency) {
            payment.amount
        } else {
            let usd = payment
                .price
                .price_usd
                .or_else(|| book.price_at(&payment.asset.key(), secs))
                .unwrap_or(Amount::ZERO);
            book.convert(payment.amount * usd, "usd", &item.currency, secs)
                .unwrap_or(Amount::ZERO)
        };
        *out.entry(calendar::month_key(cal.date_of(secs)))
            .or_default() += amount;
    }
    out
}

/// The item's payment transactions by period.
pub fn payments_by_month(
    books: &Books<'_>,
    item: &UtilityItem,
    cal: &Calendar,
) -> BTreeMap<String, Vec<Transaction>> {
    let mut out: BTreeMap<String, Vec<Transaction>> = BTreeMap::new();
    for payment in item_payments(books.doc(), item) {
        let secs = secs_or_now(&payment.meta.timestamp);
        out.entry(calendar::month_key(cal.date_of(secs)))
            .or_default()
            .push(Transaction::Payment(payment.clone()));
    }
    out
}
