//! Shim of the `insights` area: moved to `crate::reports::insights` (spec core-strangler, D1).

pub use crate::reports::insights::{
    Insight, InsightCode, OverviewFlow, OverviewInvestments, OverviewWealth, OverviewYear,
    ReportOverview, report_insights,
};

use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;
use super::reports::ReportFilter;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The overview for the filter's scope and currency. Its month is
        /// `filter.to` (this month by default), compared with the one before.
        pub fn report_overview(&self, filter: &ReportFilter, now: i64, zone: &str) -> Result<ReportOverview>
            => crate::reports::insights::report_overview @books;
    }
    // forward:end
}
