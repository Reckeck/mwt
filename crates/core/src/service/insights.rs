//! Shim of the `insights` area: moved to `mwt_reports::insights` (spec core-strangler, D1).

pub use mwt_reports::insights::{
    OverviewFlow, OverviewInvestments, OverviewWealth, OverviewYear, report_insights,
};
