//! Shim of the `investments` area: moved to `crate::money::investments`
//! (spec core-strangler, D1).

pub use crate::money::investments::{InvestmentRow, Investments};

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// `store_ids` limits it to these wallets and what is under them; empty is all.
        pub fn investments(&self, store_ids: &[String]) -> Investments
            => crate::money::investments::investments @books;
    }
    // forward:end
}
