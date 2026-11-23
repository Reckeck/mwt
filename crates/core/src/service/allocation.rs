//! Shim of the `allocation` area: the rules moved to `crate::planning::allocation`, the
//! scenarios to `crate::usecases::planning::allocation` (spec core-strangler, D1).

pub use crate::planning::allocation::{
    AssetClassTarget, AssetClassesView, PRESET_GLOBAL, PRESET_LOCAL, PRESET_RISKY, TopupAdvice,
    TopupRow, preset_classes,
};
