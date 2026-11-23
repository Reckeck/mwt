//! Shim of the `scenarios` area: the rules moved to `crate::planning::scenarios`, the
//! scenarios to `crate::usecases::planning::scenarios` (spec core-strangler, D1).

pub use crate::planning::scenarios::{
    CalcScenarioDeleted, CalcScenarioDraft, INCOME_MONTHS, ScenarioPrefill, planned_balance,
};
