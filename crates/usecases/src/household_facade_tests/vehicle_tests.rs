//! Tests of the vehicle area: the old inline tests (names and bodies unchanged),
//! then the deciders on a bare document and the delegate pins.

use crate::error::Error;
use crate::household::vehicle::*;
use crate::model::ids::StoreId;
use crate::model::store::{CarInfo, PropertyCategory, PropertyInfo, StoreKind};
use crate::service::test_support::*;
use crate::service::{Session, StoreExtras};
use crate::usecases::household::vehicle as scenarios;
use chrono::NaiveDate;

fn day(raw: &str) -> NaiveDate {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
}

fn car(vault: &mut Session, category: PropertyCategory) -> String {
    crate::usecases::money::stores::create_store_with(
        vault,
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

fn info(vault: &Session, id: &str) -> CarInfo {
    crate::money::tree::find_store(&vault.doc, id)
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
    crate::usecases::household::vehicle::vehicle_update_mileage(
        &mut vault,
        &StoreId::from(&id),
        42_000,
        day("2026-03-10"),
    )
    .unwrap();
    let read = info(&vault, &id);
    assert_eq!(read.mileage_km, Some(42_000));
    assert_eq!(read.mileage_at.as_deref(), Some("2026-03-10"));
}

#[test]
fn service_done_moves_the_next_one_by_the_intervals() {
    let (_dir, mut vault) = empty_vault();
    let id = car(&mut vault, PropertyCategory::Car);
    crate::usecases::household::vehicle::vehicle_service_done(
        &mut vault,
        &StoreId::from(&id),
        day("2026-01-31"),
        Some(45_000),
    )
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
        crate::usecases::household::vehicle::vehicle_update_mileage(
            &mut vault,
            &StoreId::from(&home),
            1,
            day("2026-03-10")
        ),
        Err(Error::InvalidInput(_))
    ));
    assert!(
        crate::usecases::household::vehicle::vehicle_update_mileage(
            &mut vault,
            &StoreId::from("nope"),
            1,
            day("2026-03-10")
        )
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

#[test]
fn deciders_run_on_a_bare_document_and_change_nothing() {
    let (_dir, mut vault) = empty_vault();
    let id = StoreId::from(car(&mut vault, PropertyCategory::Car));
    let before = vault.doc.clone();
    let read = plan_update_mileage(&vault.doc, &id, 7, day("2026-03-10")).unwrap();
    assert_eq!(read.mileage_km, Some(7));
    let done = plan_service_done(&vault.doc, &id, day("2026-01-31"), None).unwrap();
    assert_eq!(done.service_due_at.as_deref(), Some("2027-01-31"));
    assert_eq!(done.service_due_km, None, "no mileage read, no due mileage");
    assert_eq!(vault.doc, before);
}

#[test]
fn a_missing_archived_or_non_car_store_is_refused_before_any_write() {
    let (_dir, mut vault) = empty_vault();
    let home = car(&mut vault, PropertyCategory::Flat);
    vault.doc.stores.iter_mut().for_each(|store| {
        if store.id == home {
            store.name = "Flat".into();
        }
    });
    let home = StoreId::from(home);
    let parked = car(&mut vault, PropertyCategory::Car);
    vault.doc.stores.iter_mut().for_each(|store| {
        if store.id == parked {
            store.archived = true;
        }
    });
    let parked = StoreId::from(parked);
    let today = day("2026-03-10");
    assert!(matches!(
        car_info(&vault.doc, &StoreId::from("nope")),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        plan_update_mileage(&vault.doc, &home, 1, today),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        plan_service_done(&vault.doc, &parked, today, None),
        Err(Error::Archived(_))
    ));
}

#[test]
fn the_delegates_equal_the_scenarios() {
    let (_first_dir, mut delegated) = empty_vault();
    let (_second_dir, mut direct) = empty_vault();
    let first = car(&mut delegated, PropertyCategory::Car);
    let second = car(&mut direct, PropertyCategory::Car);
    let today = day("2026-03-10");
    let by_delegate = crate::usecases::household::vehicle::vehicle_update_mileage(
        &mut delegated,
        &StoreId::from(&first),
        42_000,
        today,
    )
    .unwrap();
    let by_scenario =
        scenarios::vehicle_update_mileage(&mut direct, &StoreId::from(&second), 42_000, today)
            .unwrap();
    assert_eq!(by_delegate.property, by_scenario.property);
    let by_delegate = crate::usecases::household::vehicle::vehicle_service_done(
        &mut delegated,
        &StoreId::from(&first),
        today,
        Some(45_000),
    )
    .unwrap();
    let by_scenario =
        scenarios::vehicle_service_done(&mut direct, &StoreId::from(&second), today, Some(45_000))
            .unwrap();
    assert_eq!(by_delegate.property, by_scenario.property);
}
