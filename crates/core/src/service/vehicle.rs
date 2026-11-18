//! Shim of the `vehicle` area: moved to `crate::household::vehicle` (spec core-strangler, D1).

use chrono::NaiveDate;

use crate::error::Result;
use crate::model::ids::StoreId;
use crate::model::store::Store;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The odometer read today.
        pub fn vehicle_update_mileage(&mut self, store_id: &str, km: u32, today: NaiveDate) -> Result<Store>
            => crate::usecases::household::vehicle::vehicle_update_mileage with (store_id => &StoreId::from(store_id));
        /// The service was done on `date` (at `km`, if read): the next one moves
        /// by the intervals; without an interval that side is cleared.
        pub fn vehicle_service_done(&mut self, store_id: &str, date: NaiveDate, km: Option<u32>) -> Result<Store>
            => crate::usecases::household::vehicle::vehicle_service_done with (store_id => &StoreId::from(store_id));
    }
    // forward:end
}
