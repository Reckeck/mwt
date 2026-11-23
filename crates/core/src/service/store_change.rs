//! Shim of the `store_change` area: moved to `crate::money::store_change`
//! (spec core-strangler, D1). The change is worked out there on a copy of the
//! document; the scenarios that plan and apply it live in
//! `usecases::money::store_change`.

#[cfg(test)]
pub use crate::money::store_change::StoreChangeInput;
pub use crate::money::store_change::{StoreChange, StoreChangeConflict, StoreChangeItem};
