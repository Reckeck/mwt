//! Shim of the `reports` area: moved to `mwt_reports::overview` (spec core-strangler, D1).
//! The reports take `&Books`; the delegates below keep the old `UnlockedVault` methods that
//! `insights`, `where_went`, `losses` and the household scenarios still call.

pub use mwt_reports::overview::{
    CalendarDay, CalendarMonth, CashflowReport, CashflowRow, CategoriesReport, CategoryGroupBlock,
    CategoryRow, DayStatus, InflationPoint, InflationReport, NetworthPoint, NetworthReport,
    PnlReport, PnlRow, PortfolioPoint, PortfolioReport, Report, ReportFilter, ReportKind,
    ReportMeta, StoreReportRow, StoresReport, UnpricedSpending, YearReport, YearRow, report_csv,
};
pub(crate) use mwt_reports::overview::{Ctx, csv_text};

use crate::error::Result;

use super::UnlockedVault;
use super::budget::Flow;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// One report of a kind for a filter. `inflation` is the user's
        /// reference table (`AppSettings.inflation`).
        pub fn report(&self, kind: ReportKind, filter: &ReportFilter, now: i64, zone: &str) -> Result<Report>
            => mwt_reports::overview::report @books;
        /// Spending by day of a month (`YYYY-MM`) against the day's allowance,
        /// with what is still to come (plan F6).
        pub fn calendar_month(&self, month: &str, now: i64, zone: &str) -> Result<CalendarMonth>
            => mwt_reports::overview::calendar_month @books;
        pub(crate) fn report_ctx(&self, filter: &ReportFilter, now: i64, zone: &str) -> Result<Ctx>
            => mwt_reports::overview::report_ctx @books;
        /// Classified flows of the report range (or `[from, to)`), in scope.
        pub(crate) fn report_flows(&self, ctx: &Ctx, from: i64, to: i64) -> Vec<Flow>
            => mwt_reports::overview::report_flows @books;
        pub(crate) fn cashflow_report(&self, ctx: &Ctx) -> CashflowReport
            => mwt_reports::overview::cashflow_report @books;
    }
    // forward:end
}
