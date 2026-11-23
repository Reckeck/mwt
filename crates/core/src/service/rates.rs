//! Shim of the `rates` area: moved to `crate::money::rates` (spec core-strangler, D1).
//! The scenarios that write the rate caches or speak the wire types live in
//! `usecases::money::rates`.

use crate::model::transaction::Transaction;
use crate::money::rates::RateBook;

use super::UnlockedVault;

impl UnlockedVault {
    /// Give every unpriced side the USD price of its moment (plan §6.2).
    pub(crate) fn stamp_prices(book: &RateBook, tx: &mut Transaction) -> usize {
        crate::money::rates::stamp_prices(book, tx)
    }
}
