//! Shim of the `health` area: the figures moved to `crate::planning::health`, the light, the
//! report and the scenarios to `crate::usecases::planning::health` (spec core-strangler, D1).

pub use crate::planning::health::{
    DTI_ELEVATED_PCT, DTI_HIGH_PCT, FUND_MIN_MONTHS, FUND_SAFE_MONTHS, HealthReason,
    SAVINGS_BENCHMARK_PCT, SavingsMonth,
};
pub use mwt_reports::health::HealthReport;
