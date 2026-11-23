//! Shim of the `adjustments` area: moved to `crate::money::adjustments` (spec
//! core-strangler, D1). The corrections are worked out there; the scenario that
//! rebases them over the vault is in `usecases::money::adjustments`.

pub use crate::money::adjustments::AdjustmentDraft;
