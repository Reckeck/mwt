//! Shim of the `scenarios` area: the rules moved to `crate::planning::scenarios`, the
//! scenarios to `crate::usecases::planning::scenarios` (spec core-strangler, D1).

pub use crate::planning::scenarios::{INCOME_MONTHS, planned_balance};
