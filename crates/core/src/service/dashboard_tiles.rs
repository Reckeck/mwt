//! Shim of the `dashboard_tiles` area: moved to `mwt_reports::dashboard_tiles` (spec core-strangler, D1).

pub use mwt_reports::dashboard_tiles::{CategoryGrowth, MonthPace, NetWorthTrend};

use super::UnlockedVault;
use super::forward::forward;
use crate::model::vault::DashPeriod;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The current month's pace (plan H3: "Daily budget" and "This month").
        pub fn month_pace(&self, now: i64, zone: &str) -> MonthPace
            => mwt_reports::dashboard_tiles::month_pace @books;
        /// The holdings' value over a period (plan H3: "Net worth").
        pub fn net_worth_trend(&self, period: DashPeriod, now: i64, zone: &str) -> NetWorthTrend
            => mwt_reports::dashboard_tiles::net_worth_trend @books;
    }
    // forward:end
}
