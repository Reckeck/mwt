//! Shim of the `dashboard` area: moved to `crate::reports::dashboard` (spec core-strangler, D1).

pub use crate::reports::dashboard::{SpendingDay, SpendingOverview};

use super::UnlockedVault;
use super::forward::forward;
use crate::model::vault::DashPeriod;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Spending for the dashboard's "Spending" (plan H2).
        pub fn spending_overview(&self, period: DashPeriod, now: i64, zone: &str) -> SpendingOverview
            => crate::reports::dashboard::spending_overview @books;
    }
    // forward:end
}
