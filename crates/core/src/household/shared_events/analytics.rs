//! Valuing the spending of shared events (the entries are collected by
//! `money::shared_ratios`): the category group of an entry, its value in the
//! currency asked, and the entries of a period. The budget row made of them is
//! the scenario in `usecases::household::shared_events`.

use crate::Amount;
use crate::model::VaultDoc;
use crate::model::category::CategoryGroup;
use crate::money::rates::RateBook;
use crate::money::shared_ratios::SharedExtra;

/// The group of a category of the vault; `None` when the vault does not know it.
pub(crate) fn category_group(doc: &VaultDoc, category: &str) -> Option<CategoryGroup> {
    doc.categories
        .iter()
        .find(|known| known.key == category)
        .map(|known| known.group)
}

/// The entry in the event currency valued in `currency`; when a side has no
/// rate: zero and the event currency (upper case) as the missing one.
pub(crate) fn value_in(
    book: &RateBook,
    extra: &SharedExtra,
    currency: &str,
) -> (Amount, Vec<String>) {
    match book.convert(extra.value, &extra.event_currency, currency, extra.secs) {
        Some(value) => (value, Vec::new()),
        None => (Amount::ZERO, vec![extra.event_currency.to_uppercase()]),
    }
}

/// The entries dated in `[from, to)`.
pub(crate) fn extras_between(
    extras: &[SharedExtra],
    from: i64,
    to: i64,
) -> impl Iterator<Item = &SharedExtra> {
    extras
        .iter()
        .filter(move |extra| extra.secs >= from && extra.secs < to)
}
