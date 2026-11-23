//! Shim of the `emergency` area: the rules moved to `crate::planning::emergency`, the scenarios to
//! `crate::usecases::planning::emergency` (spec core-strangler, D1).

pub use crate::planning::emergency::{EmergencyStatus, EmergencyStore, MAX_FUND_MONTHS};
