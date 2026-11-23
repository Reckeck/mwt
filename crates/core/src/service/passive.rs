//! Shim of the `passive` area: the rules moved to `crate::planning::passive`, the default
//! categories to `crate::usecases::planning::passive` (spec core-strangler, D1).

pub use crate::planning::passive::{PassiveIncome, PassiveMonth, PassiveShare};
