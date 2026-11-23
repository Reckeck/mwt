//! Shim of the `health` area: the figures moved to `crate::planning::health`, the light, the
//! report and the scenarios to `crate::usecases::planning::health` (spec core-strangler, D1).

pub use crate::planning::health::{
    DTI_ELEVATED_PCT, DTI_HIGH_PCT, FUND_MIN_MONTHS, FUND_SAFE_MONTHS, HealthReason,
    SAVINGS_BENCHMARK_PCT, SavingsMonth,
};
pub use mwt_reports::health::HealthReport;

use crate::Amount;
use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;
use super::insights::Insight;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The savings rate of the last finished months, oldest first.
        pub fn savings_series(&self, now: i64, zone: &str) -> Result<Vec<SavingsMonth>>
            => crate::planning::health::savings_series @books;
        /// Spending growing as fast as income or faster (decision 26): the last
        /// finished months against the same months a year back, or against the
        /// months just before when the year has no data.
        pub fn lifestyle_creep(&self, now: i64, zone: &str) -> Option<Insight>
            => mwt_reports::health::lifestyle_creep @books;
        /// The checkup. `manual` is the hand-entered inflation table, as for the
        /// other inflation readers.
        pub fn capital_health(&self, now: i64, zone: &str) -> Result<HealthReport>
            => mwt_reports::health::capital_health @books;
        /// Set the yearly return the freedom potential assumes; `None` = the key rate.
        pub fn set_freedom_rate(&mut self, rate: Option<Amount>) -> Result<()>
            => crate::usecases::planning::health::set_freedom_rate;
    }
    // forward:end

    // Called by the notification sweep until it moves.
    forward! {}
}
