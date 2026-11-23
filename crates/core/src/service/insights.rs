//! Shim of the `insights` area: moved to `mwt_reports::insights` (spec core-strangler, D1).

pub use mwt_reports::insights::{
    Insight, InsightCode, OverviewFlow, OverviewInvestments, OverviewWealth, OverviewYear,
    ReportOverview, report_insights,
};
