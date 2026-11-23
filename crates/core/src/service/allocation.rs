//! Shim of the `allocation` area: the rules moved to `crate::planning::allocation`, the
//! scenarios to `crate::usecases::planning::allocation` (spec core-strangler, D1).

#[cfg(test)]
pub use crate::planning::allocation::AssetClassTarget;
pub use crate::planning::allocation::{
    PRESET_GLOBAL, PRESET_LOCAL, PRESET_RISKY, TopupRow, preset_classes,
};
