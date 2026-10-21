//! A car's mileage and service (plan G2): the two actions the car card offers.
//! The reminders follow by themselves (`auto_reminders`).

use chrono::{Months, NaiveDate};

use crate::model::store::{CarInfo, PropertyCategory, Store, StoreKind};
use crate::{Error, Result};

use super::UnlockedVault;

impl UnlockedVault {
    /// The car's own info, created empty on first use.
    fn car_mut(&mut self, store_id: &str) -> Result<&mut CarInfo> {
        let store = self
            .doc
            .stores
            .iter_mut()
            .find(|candidate| candidate.id == store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        let info = store
            .property
            .as_mut()
            .filter(|info| {
                store.kind == StoreKind::Property && info.category == PropertyCategory::Car
            })
            .ok_or_else(|| Error::InvalidInput("not a car".into()))?;
        if store.archived {
            return Err(Error::Archived(store.name.clone()));
        }
        Ok(info.car.get_or_insert_with(CarInfo::default))
    }

    fn saved_store(&self, store_id: &str) -> Result<Store> {
        self.find_store(store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))
    }

    /// The odometer read today.
    pub fn vehicle_update_mileage(
        &mut self,
        store_id: &str,
        km: u32,
        today: NaiveDate,
    ) -> Result<Store> {
        let car = self.car_mut(store_id)?;
        car.mileage_km = Some(km);
        car.mileage_at = Some(today.format("%Y-%m-%d").to_string());
        self.persist()?;
        self.saved_store(store_id)
    }

    /// The service was done on `date` (at `km`, if read): the next one moves
    /// by the intervals; without an interval that side is cleared.
    pub fn vehicle_service_done(
        &mut self,
        store_id: &str,
        date: NaiveDate,
        km: Option<u32>,
    ) -> Result<Store> {
        let car = self.car_mut(store_id)?;
        if let Some(km) = km {
            car.mileage_km = Some(km);
            car.mileage_at = Some(date.format("%Y-%m-%d").to_string());
        }
        car.service_due_at = car
            .service_interval_months
            .filter(|months| *months > 0)
            .and_then(|months| date.checked_add_months(Months::new(u32::from(months))))
            .map(|next| next.format("%Y-%m-%d").to_string());
        car.service_due_km = match (
            car.service_interval_km.filter(|step| *step > 0),
            car.mileage_km,
        ) {
            (Some(step), Some(now)) => Some(now.saturating_add(step)),
            _ => None,
        };
        self.persist()?;
        self.saved_store(store_id)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::model::store::PropertyInfo;
    use crate::service::StoreExtras;

    fn day(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    fn car(vault: &mut UnlockedVault, category: PropertyCategory) -> String {
        vault
            .create_store_with(
                "Октавия",
                StoreKind::Property,
                "#555",
                "",
                None,
                StoreExtras {
                    property: Some(PropertyInfo {
                        category,
                        car: Some(CarInfo {
                            service_interval_months: Some(12),
                            service_interval_km: Some(15_000),
                            ..CarInfo::default()
                        }),
                        ..PropertyInfo::default()
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id
    }

    fn info(vault: &UnlockedVault, id: &str) -> CarInfo {
        vault
            .find_store(id)
            .unwrap()
            .property
            .clone()
            .unwrap()
            .car
            .unwrap()
    }

    #[test]
    fn mileage_is_read_with_its_date() {
        let (_dir, mut vault) = empty_vault();
        let id = car(&mut vault, PropertyCategory::Car);
        vault
            .vehicle_update_mileage(&id, 42_000, day("2026-03-10"))
            .unwrap();
        let read = info(&vault, &id);
        assert_eq!(read.mileage_km, Some(42_000));
        assert_eq!(read.mileage_at.as_deref(), Some("2026-03-10"));
    }

    #[test]
    fn service_done_moves_the_next_one_by_the_intervals() {
        let (_dir, mut vault) = empty_vault();
        let id = car(&mut vault, PropertyCategory::Car);
        vault
            .vehicle_service_done(&id, day("2026-01-31"), Some(45_000))
            .unwrap();
        let done = info(&vault, &id);
        assert_eq!(done.service_due_at.as_deref(), Some("2027-01-31"));
        assert_eq!(done.service_due_km, Some(60_000));
        assert_eq!(done.mileage_km, Some(45_000));
        assert!(!done.service_due_by_km());
    }

    #[test]
    fn only_a_car_has_a_mileage() {
        let (_dir, mut vault) = empty_vault();
        let home = car(&mut vault, PropertyCategory::Flat);
        assert!(matches!(
            vault.vehicle_update_mileage(&home, 1, day("2026-03-10")),
            Err(Error::InvalidInput(_))
        ));
        assert!(
            vault
                .vehicle_update_mileage("nope", 1, day("2026-03-10"))
                .is_err()
        );
    }

    #[test]
    fn an_old_property_without_a_car_reads_and_a_house_is_a_home() {
        let old: PropertyInfo = serde_json::from_str(
            r#"{"category":"house","address":"","area":null,"purchased_at":null,"purchase_price":null,"currency":"rub"}"#,
        )
        .unwrap();
        assert!(old.car.is_none());
        assert!(old.category.is_home());
        assert_eq!(
            old.category.automation_presets(),
            PropertyCategory::Flat.automation_presets()
        );
    }
}
