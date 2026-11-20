//! Shim of the `debts` area: moved to `crate::household::debts` (spec core-strangler, D1).

pub use crate::household::debts::{DebtLoad, LoanClass, LoanStatus, classify_loan};

use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Every live loan and credit card with a debt or a payment, their class
        /// and DTI. `manual` is the hand-entered inflation table.
        pub fn debt_load(&self, now: i64, zone: &str) -> Result<DebtLoad>
            => crate::planning::debt_load::debt_load @books;
    }
    // forward:end
}
