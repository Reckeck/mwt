//! Shim of the `loyalty` area: moved to `crate::household::loyalty` (spec core-strangler, D1).

pub use crate::household::loyalty::{CashbackReport, CashbackSource, LoyaltyBalance};

use crate::Amount;
use crate::error::Result;
use crate::model::store::{LoyaltyInfo, Store};
use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The programme shares its symbol with another one. Computed for the
        /// view, never stored; the form asks to rename.
        pub fn loyalty_symbol_clash(&self, store_id: &str) -> bool
            => crate::household::loyalty::loyalty_symbol_clash @doc;
        /// Cashback (income in the `cashback` category) and points credited to
        /// bonus programmes between two moments (RFC3339, inclusive), by card.
        pub fn cashback_report(&self, from: &str, to: &str) -> CashbackReport
            => crate::household::loyalty::cashback_report @books;
        /// Set a programme's points and price of a point. The price becomes the
        /// points' hand-set price, so the wallet counts in net worth.
        pub fn set_loyalty_info(&mut self, store_id: &str, info: LoyaltyInfo) -> Result<Store>
            => crate::usecases::household::loyalty::set_loyalty_info;
        /// Create a bonus programme and set its points in one action, so a
        /// programme never exists without them.
        pub fn create_loyalty_program(&mut self, name: &str, color: &str, contact_id: Option<String>, info: LoyaltyInfo) -> Result<Store>
            => crate::usecases::household::loyalty::create_loyalty_program;
        /// Credit (`delta` > 0) or spend points of a programme, optionally tied
        /// to the card whose purchase earned them.
        pub fn adjust_points(&mut self, store_id: &str, delta: Amount, comment: &str, source_store_id: Option<String>, timestamp: Option<String>) -> Result<Transaction>
            => crate::usecases::household::loyalty::adjust_points;
    }
    // forward:end
}
