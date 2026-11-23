//! Shim of the `inflation` area: moved to `crate::money::inflation` (spec core-strangler, D1).
//! The scenarios and the notices live in `usecases::money::inflation`.

pub use crate::money::inflation::{INFLATION_CHART_MONTHS, InflationNews};
pub use crate::usecases::money::inflation::{inflation_notice, key_rate_notice};
