//! A car's mileage and service (plan G2): the two actions the car card offers.
//! The reminders follow by themselves (`auto_reminders`).
//!
//! Pure part of the area: each decider reads a document and returns the car's
//! new info. The write is the scenario in `usecases::household::vehicle`.

use chrono::{Months, NaiveDate};

use crate::error::{Error, Result};
use crate::model::ids::StoreId;
use crate::model::store::{CarInfo, PropertyCategory, StoreKind};
use crate::model::vault::VaultDoc;
use crate::money::tree::find_store;

fn date_key(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// The car's own info as it is now (empty on first use); a refusal when the
/// store is missing, is not a car or is archived.
pub fn car_info(doc: &VaultDoc, store_id: &StoreId) -> Result<CarInfo> {
    let store = find_store(doc, store_id.as_str())
        .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
    let info = store
        .property
        .as_ref()
        .filter(|info| store.kind == StoreKind::Property && info.category == PropertyCategory::Car)
        .ok_or_else(|| Error::InvalidInput("not a car".into()))?;
    if store.archived {
        return Err(Error::Archived(store.name.clone()));
    }
    Ok(info.car.clone().unwrap_or_default())
}

/// The odometer read today: the car's info to write.
pub fn plan_update_mileage(
    doc: &VaultDoc,
    store_id: &StoreId,
    km: u32,
    today: NaiveDate,
) -> Result<CarInfo> {
    let mut car = car_info(doc, store_id)?;
    car.mileage_km = Some(km);
    car.mileage_at = Some(date_key(today));
    Ok(car)
}

/// The service was done on `date` (at `km`, if read): the next one moves
/// by the intervals; without an interval that side is cleared.
pub fn plan_service_done(
    doc: &VaultDoc,
    store_id: &StoreId,
    date: NaiveDate,
    km: Option<u32>,
) -> Result<CarInfo> {
    let mut car = car_info(doc, store_id)?;
    if let Some(km) = km {
        car.mileage_km = Some(km);
        car.mileage_at = Some(date_key(date));
    }
    car.service_due_at = car
        .service_interval_months
        .filter(|months| *months > 0)
        .and_then(|months| date.checked_add_months(Months::new(u32::from(months))))
        .map(date_key);
    car.service_due_km = match (
        car.service_interval_km.filter(|step| *step > 0),
        car.mileage_km,
    ) {
        (Some(step), Some(now)) => Some(now.saturating_add(step)),
        _ => None,
    };
    Ok(car)
}
