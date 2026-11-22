//! Shim of the `dashboard` area: moved to `mwt_reports::dashboard` (spec core-strangler, D1).

pub use mwt_reports::dashboard::{SpendingDay, SpendingOverview};

use super::UnlockedVault;
use super::forward::forward;
use crate::model::vault::DashPeriod;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Spending for the dashboard's "Spending" (plan H2).
        pub fn spending_overview(&self, period: DashPeriod, now: i64, zone: &str) -> SpendingOverview
            => mwt_reports::dashboard::spending_overview @books;
    }
    // forward:end
}
