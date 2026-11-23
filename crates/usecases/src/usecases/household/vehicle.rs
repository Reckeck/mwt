//! Scenarios of the vehicle area: each decides in `household::vehicle`, then
//! ends in one `vault.transact` that writes the car's info and returns the store.

use chrono::NaiveDate;

use crate::error::{Error, Result};
use crate::household::vehicle::{plan_service_done, plan_update_mileage};
use crate::model::ids::StoreId;
use crate::model::store::{CarInfo, Store};
use crate::service::Session;

/// The odometer read today.
pub fn vehicle_update_mileage(
    vault: &mut Session,
    store_id: &StoreId,
    km: u32,
    today: NaiveDate,
) -> Result<Store> {
    let car = plan_update_mileage(&vault.doc, store_id, km, today)?;
    write_car(vault, store_id, car)
}

/// The service was done on `date` (at `km`, if read): the next one moves
/// by the intervals; without an interval that side is cleared.
pub fn vehicle_service_done(
    vault: &mut Session,
    store_id: &StoreId,
    date: NaiveDate,
    km: Option<u32>,
) -> Result<Store> {
    let car = plan_service_done(&vault.doc, store_id, date, km)?;
    write_car(vault, store_id, car)
}

fn write_car(vault: &mut Session, store_id: &StoreId, car: CarInfo) -> Result<Store> {
    vault.transact(|tx| {
        tx.collection::<Store>()
            .update(store_id.as_str(), |store| {
                if let Some(info) = store.property.as_mut() {
                    info.car = Some(car);
                }
                store.clone()
            })
            .map_err(|_| Error::Missing(format!("store {store_id}")))
    })
}
