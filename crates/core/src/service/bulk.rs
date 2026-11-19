//! Shim of the `bulk` area: moved to `crate::money::bulk` (spec core-strangler,
//! D1). The scenarios are in `usecases::money::bulk`.

pub use crate::money::bulk::{DeletedTransaction, TxCategories, TxStore};

use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Delete every operation of `ids`; an unknown id deletes none.
        pub fn delete_transactions(&mut self, ids: &[String]) -> Result<Vec<DeletedTransaction>>
            => crate::usecases::money::bulk::delete_transactions;
        /// Put deleted operations back as they were: same id, origin and property.
        pub fn restore_transactions(&mut self, items: Vec<DeletedTransaction>) -> Result<()>
            => crate::usecases::money::bulk::restore_transactions;
        /// Replace the categories of each operation; returns the ones they had.
        pub fn set_transaction_categories(&mut self, changes: Vec<TxCategories>) -> Result<Vec<TxCategories>>
            => crate::usecases::money::bulk::set_transaction_categories;
        /// Book payments and top-ups on another wallet; returns the wallets they had.
        /// Two-sided kinds (transfer, exchange) are refused: which side would move?
        pub fn move_transactions(&mut self, changes: Vec<TxStore>) -> Result<Vec<TxStore>>
            => crate::usecases::money::bulk::move_transactions;
    }
    // forward:end
}
