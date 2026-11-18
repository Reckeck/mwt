//! Shim of the `store_months` area: moved to `crate::money::store_months` (spec core-strangler, D1).

pub use crate::money::store_months::StoreMonthRow;

use crate::model::ids::StoreId;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The last `months` local months up to the one holding `now`, oldest first.
        /// Moves between two stores of the scope are left out: they are neither.
        pub fn store_months(&self, store_id: &str, include_children: bool, months: u32, now: i64, zone: &str) -> Vec<StoreMonthRow>
            => crate::usecases::household::store_months::store_months with (store_id => &StoreId::from(store_id));
    }
    // forward:end
}
