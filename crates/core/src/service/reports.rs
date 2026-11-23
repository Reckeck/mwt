//! Shim of the `reports` area: moved to `mwt_reports::overview` (spec core-strangler, D1).
//! The reports take `&Books`.

pub use mwt_reports::overview::{
    CalendarDay, CalendarMonth, CashflowReport, CashflowRow, CategoriesReport, CategoryGroupBlock,
    CategoryRow, DayStatus, InflationPoint, InflationReport, NetworthPoint, NetworthReport,
    PnlReport, PnlRow, PortfolioPoint, PortfolioReport, Report, ReportFilter, ReportKind,
    ReportMeta, StoreReportRow, StoresReport, UnpricedSpending, YearReport, YearRow, report_csv,
};
