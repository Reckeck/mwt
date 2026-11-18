//! Shim of the `recurring` area: moved to `crate::planning::recurring` (spec core-strangler, D1).

pub use crate::planning::recurring::RecurringCheck;

use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Compares a payment with the last earlier one to the same payee, in the
        /// same first category and asset, within 45 days. Anything but a payment
        /// counts as new.
        pub fn recurring_check(&self, tx: &Transaction) -> RecurringCheck
            => crate::planning::recurring::recurring_check @doc;
    }
    // forward:end
}
