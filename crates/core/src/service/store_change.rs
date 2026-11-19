//! Shim of the `store_change` area: moved to `crate::money::store_change`
//! (spec core-strangler, D1). The change is worked out there on a copy of the
//! document; the scenarios that plan and apply it live in
//! `usecases::money::store_change`.

pub use crate::money::store_change::{
    StoreChange, StoreChangeConflict, StoreChangeInput, StoreChangeItem, StoreChangePlan,
};

use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// What a change would do; nothing is written.
        pub fn plan_store_change(&mut self, input: &StoreChangeInput) -> StoreChangePlan
            => crate::usecases::money::store_change::plan_store_change;
        /// Make the change: refused while it has an error or a conflict. The vault
        /// file is copied to `.bak` first.
        pub fn apply_store_change(&mut self, input: &StoreChangeInput) -> Result<StoreChangePlan>
            => crate::usecases::money::store_change::apply_store_change;
    }
    // forward:end
}
