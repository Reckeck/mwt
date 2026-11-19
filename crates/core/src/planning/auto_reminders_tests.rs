//! Tests of the auto_reminders area: the old inline tests (names and bodies unchanged).

use super::auto_reminders::*;
use crate::model::notify::NoticeCategory;
use crate::model::planning::{Reminder, ReminderSourceKind};
use crate::model::planning::{RepeatUnit, UtilityKind};
use crate::model::store::StoreKind;
use crate::pricing::history::secs_or_now;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use chrono::NaiveDate;

use crate::model::store::{LiabilityKind, LiabilityTerms, PropertyInfo};
use crate::model::transaction::AccountRef;
use crate::service::{StoreExtras, UtilityDraft};

fn loan(vault: &mut UnlockedVault, due_day: u8) -> String {
    vault
        .create_store_with(
            "Ипотека",
            StoreKind::Liability,
            "#c00",
            "",
            None,
            StoreExtras {
                liability: Some(LiabilityTerms {
                    kind: LiabilityKind::Mortgage,
                    rate: None,
                    limit: None,
                    due_day: Some(due_day),
                    monthly_payment: Some(amount("45000")),
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id
}

fn auto(vault: &UnlockedVault, kind: ReminderSourceKind) -> Vec<Reminder> {
    vault
        .doc
        .reminders
        .iter()
        .filter(|reminder| {
            reminder
                .source
                .as_ref()
                .is_some_and(|source| source.kind == kind)
        })
        .cloned()
        .collect()
}

#[test]
fn a_loan_gets_a_monthly_reminder_ahead_of_its_payment_day() {
    let (_dir, mut vault) = empty_vault();
    let id = loan(&mut vault, 25);
    let now = secs_or_now("2026-03-10T08:00:00Z");
    assert!(vault.sync_auto_reminders(now, "UTC", true).unwrap());
    let reminders = auto(&vault, ReminderSourceKind::LoanPayment);
    assert_eq!(reminders.len(), 1);
    // One day ahead (the loan category's default), at 10:00.
    assert_eq!(reminders[0].due_at, "2026-03-24T10:00:00Z");
    assert_eq!(
        reminders[0].repeat.map(|repeat| repeat.unit),
        Some(RepeatUnit::Month)
    );
    assert!(reminders[0].text.contains("Ипотека") && reminders[0].text.contains("45000"));
    assert_eq!(reminders[0].source.as_ref().unwrap().id, id);

    // Nothing to do the second time.
    assert!(!vault.sync_auto_reminders(now, "UTC", true).unwrap());
}

#[test]
fn a_changed_payment_day_moves_it_but_done_is_not_undone() {
    let (_dir, mut vault) = empty_vault();
    let id = loan(&mut vault, 25);
    let now = secs_or_now("2026-03-10T08:00:00Z");
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    let reminder = auto(&vault, ReminderSourceKind::LoanPayment)[0].clone();

    // Done: it moves to April, and the next sync keeps it there.
    vault.set_reminder_done(&reminder.id, true).unwrap();
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    let after = auto(&vault, ReminderSourceKind::LoanPayment)[0].clone();
    assert_ne!(after.due_at, "2026-03-24T10:00:00Z");

    // A new payment day: placed again.
    let store = vault
        .doc
        .stores
        .iter_mut()
        .find(|store| store.id == id)
        .unwrap();
    store.liability.as_mut().unwrap().due_day = Some(15);
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    assert_eq!(
        auto(&vault, ReminderSourceKind::LoanPayment)[0].due_at,
        "2026-03-14T10:00:00Z"
    );
}

#[test]
fn an_archived_loan_loses_its_reminder_and_a_muted_one_never_returns() {
    let (_dir, mut vault) = empty_vault();
    let id = loan(&mut vault, 25);
    let now = secs_or_now("2026-03-10T08:00:00Z");
    vault.sync_auto_reminders(now, "UTC", false).unwrap();
    let reminder = auto(&vault, ReminderSourceKind::LoanPayment)[0].clone();
    vault.finish_reminder_series(&reminder.id).unwrap();
    vault.sync_auto_reminders(now, "UTC", false).unwrap();
    let kept = auto(&vault, ReminderSourceKind::LoanPayment);
    assert_eq!(kept.len(), 1);
    assert!(
        kept[0].muted && kept[0].done,
        "a finished one stays finished"
    );

    vault.archive_store(&id, true).unwrap();
    vault.sync_auto_reminders(now, "UTC", false).unwrap();
    assert!(auto(&vault, ReminderSourceKind::LoanPayment).len() <= 1);
    // Unmuted ones go with their source.
    vault
        .doc
        .reminders
        .iter_mut()
        .for_each(|reminder| reminder.muted = false);
    vault.sync_auto_reminders(now, "UTC", false).unwrap();
    assert!(auto(&vault, ReminderSourceKind::LoanPayment).is_empty());
}

#[test]
fn a_second_sync_with_nothing_new_changes_nothing() {
    let (_dir, mut vault) = empty_vault();
    let flat = store(&mut vault, "Flat", StoreKind::Property, None);
    if let Some(found) = vault.doc.stores.iter_mut().find(|known| known.id == flat) {
        found.property = Some(PropertyInfo {
            utilities_pay_day: Some(17),
            ..PropertyInfo::default()
        });
    }
    for (name, kind) in [
        (
            "Rent",
            UtilityKind::Housing {
                amount: amount("12000"),
                deposit: None,
                due_day: 17,
                automation_id: None,
            },
        ),
        (
            "Power",
            UtilityKind::Metered {
                unit: "kWh".into(),
                tariff: amount("5"),
                formula: None,
                zones: Vec::new(),
            },
        ),
    ] {
        vault
            .create_utility_item(UtilityDraft {
                name: name.into(),
                active: true,
                kind,
                currency: "thb".into(),
                property_id: flat.clone(),
                store_id: None,
                category: "utilities".into(),
            })
            .unwrap();
    }
    let power = vault.doc.utility_items[1].id.clone();
    vault
        .add_meter_reading(&power, amount("2531.5"), "2026-10", Some(5), None)
        .unwrap();
    let now = secs_or_now("2026-10-12T05:00:00Z");
    assert!(
        vault
            .sync_auto_reminders(now, "Asia/Bangkok", true)
            .unwrap()
    );
    for round in 0..3 {
        assert!(
            !vault
                .sync_auto_reminders(now + round * 30, "Asia/Bangkok", true)
                .unwrap(),
            "round {round}: {:#?}",
            vault.doc.reminders
        );
    }
}

#[test]
fn a_deleted_automatic_reminder_does_not_come_back() {
    let (_dir, mut vault) = empty_vault();
    loan(&mut vault, 25);
    let now = secs_or_now("2026-03-10T08:00:00Z");
    vault.sync_auto_reminders(now, "UTC", false).unwrap();
    let reminder = auto(&vault, ReminderSourceKind::LoanPayment)[0].clone();
    vault.delete_reminder(&reminder.id).unwrap();
    vault.sync_auto_reminders(now, "UTC", false).unwrap();
    let kept = auto(&vault, ReminderSourceKind::LoanPayment);
    assert_eq!(kept.len(), 1, "not made again");
    assert!(kept[0].muted && kept[0].done);

    // A reminder of the user's own is removed for real.
    let own = vault
        .create_reminder("call the bank", "2026-03-12T10:00:00Z", None)
        .unwrap();
    vault.delete_reminder(&own.id).unwrap();
    assert!(vault.reminders().iter().all(|known| known.id != own.id));
}

#[test]
fn a_payment_made_closes_the_month() {
    let (_dir, mut vault) = empty_vault();
    let id = loan(&mut vault, 25);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let mut salary = income(&card, "1000", "salary");
    at(&mut salary, "2026-03-01T10:00:00Z");
    vault.add_transaction(salary).unwrap();
    let now = secs_or_now("2026-03-20T08:00:00Z");
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    assert_eq!(
        auto(&vault, ReminderSourceKind::LoanPayment)[0].due_at,
        "2026-03-24T10:00:00Z"
    );

    let mut paid = transfer_between(AccountRef::Store(card), AccountRef::Store(id), usd(), "450");
    at(&mut paid, "2026-03-20T09:00:00Z");
    vault.add_transaction(paid).unwrap();
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    assert_eq!(
        auto(&vault, ReminderSourceKind::LoanPayment)[0].due_at,
        "2026-04-24T10:00:00Z"
    );
}

#[test]
fn a_switched_off_category_makes_no_reminders_and_taxes_need_a_reason() {
    let (_dir, mut vault) = empty_vault();
    loan(&mut vault, 25);
    let now = secs_or_now("2026-03-10T08:00:00Z");
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    assert!(
        auto(&vault, ReminderSourceKind::PropertyTax).is_empty(),
        "no property, no tax"
    );

    vault
        .doc
        .settings
        .notifications
        .categories
        .push(crate::model::notify::NoticeCategoryCfg {
            category: NoticeCategory::Loan,
            enabled: true,
            lead_days: 1,
            auto_reminders: false,
        });
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    assert!(auto(&vault, ReminderSourceKind::LoanPayment).is_empty());

    let flat = vault
        .create_store_with(
            "Квартира",
            StoreKind::Property,
            "#555",
            "",
            None,
            StoreExtras {
                property: Some(PropertyInfo {
                    utilities_pay_day: Some(10),
                    ..PropertyInfo::default()
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap();
    vault
        .create_utility_item(UtilityDraft {
            name: "Свет".into(),
            active: true,
            kind: UtilityKind::Metered {
                unit: "kWh".into(),
                tariff: amount("5"),
                formula: None,
                zones: Vec::new(),
            },
            currency: "rub".into(),
            property_id: flat.id.clone(),
            store_id: None,
            category: "utilities".into(),
        })
        .unwrap();
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    assert_eq!(auto(&vault, ReminderSourceKind::PropertyTax).len(), 1);
    assert_eq!(auto(&vault, ReminderSourceKind::Utilities).len(), 1);
    assert_eq!(auto(&vault, ReminderSourceKind::MeterReadings).len(), 1);
}

fn property(vault: &mut UnlockedVault, name: &str, info: PropertyInfo) -> String {
    vault
        .create_store_with(
            name,
            StoreKind::Property,
            "#555",
            "",
            None,
            StoreExtras {
                property: Some(info),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id
}

#[test]
fn each_object_gets_the_tax_of_its_type() {
    use crate::model::store::PropertyCategory;
    let (_dir, mut vault) = empty_vault();
    let home = property(
        &mut vault,
        "Дом",
        PropertyInfo {
            category: PropertyCategory::House,
            ..PropertyInfo::default()
        },
    );
    let car = property(
        &mut vault,
        "Октавия",
        PropertyInfo {
            category: PropertyCategory::Car,
            ..PropertyInfo::default()
        },
    );
    let land = property(
        &mut vault,
        "Дача",
        PropertyInfo {
            category: PropertyCategory::Land,
            ..PropertyInfo::default()
        },
    );
    property(
        &mut vault,
        "Лодка",
        PropertyInfo {
            category: PropertyCategory::Other,
            ..PropertyInfo::default()
        },
    );
    vault
        .sync_auto_reminders(secs_or_now("2026-03-10T08:00:00Z"), "UTC", true)
        .unwrap();
    let taxes = auto(&vault, ReminderSourceKind::PropertyTax);
    assert_eq!(taxes.len(), 3, "other has no tax");
    let text_of = |id: &str| {
        taxes
            .iter()
            .find(|reminder| reminder.source.as_ref().unwrap().id == id)
            .unwrap()
            .text
            .clone()
    };
    assert!(text_of(&home).starts_with("Налог на имущество: Дом"));
    assert!(text_of(&car).starts_with("Транспортный налог: Октавия"));
    assert!(text_of(&land).starts_with("Земельный налог: Дача"));
    // 14 days ahead of December 1.
    assert!(
        taxes
            .iter()
            .all(|reminder| reminder.due_at.starts_with("2026-11-17"))
    );
}

#[test]
fn a_car_is_reminded_of_its_insurance_and_service_whichever_comes_first() {
    use crate::model::store::{CarInfo, PropertyCategory};
    let (_dir, mut vault) = empty_vault();
    let car = property(
        &mut vault,
        "Октавия",
        PropertyInfo {
            category: PropertyCategory::Car,
            car: Some(CarInfo {
                osago_until: Some("2026-06-01".into()),
                kasko_until: Some("2025-01-01".into()),
                service_due_at: Some("2026-09-01".into()),
                service_due_km: Some(60_000),
                mileage_km: Some(50_000),
                mileage_at: Some("2026-03-01".into()),
                ..CarInfo::default()
            }),
            ..PropertyInfo::default()
        },
    );
    let now = secs_or_now("2026-03-10T08:00:00Z");
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    let insurance = auto(&vault, ReminderSourceKind::VehicleInsurance);
    assert_eq!(insurance.len(), 1, "an expired policy is not reminded");
    assert_eq!(
        insurance[0].source.as_ref().unwrap().id,
        format!("{car}:osago")
    );
    let service = auto(&vault, ReminderSourceKind::VehicleService);
    let by_date = service[0].due_at.clone();
    assert!(
        by_date.starts_with("2026-08-18"),
        "by date, 14 days ahead: far by km"
    );
    assert_eq!(
        category_of(ReminderSourceKind::VehicleService),
        NoticeCategory::Vehicle
    );

    // 59 500 km: the mileage says now, earlier than the date.
    let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
    vault.vehicle_update_mileage(&car, 59_500, today).unwrap();
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    let service = auto(&vault, ReminderSourceKind::VehicleService);
    assert_eq!(service.len(), 1);
    assert!(
        service[0].due_at < by_date,
        "the mileage moved it earlier: {}",
        service[0].due_at
    );
}

// -- the language of the stored text (33) --------------------------------

const MARCH: &str = "2026-03-10T08:00:00Z";

fn loan_text(vault: &UnlockedVault) -> String {
    auto(vault, ReminderSourceKind::LoanPayment)[0].text.clone()
}

#[test]
fn opening_the_list_in_another_language_leaves_the_stored_text_alone() {
    let (_dir, mut vault) = empty_vault();
    loan(&mut vault, 25);
    let now = secs_or_now(MARCH);
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    let stored = loan_text(&vault);
    assert!(stored.contains("Ипотека"));
    // A device in English opens the list: nothing is rewritten.
    assert!(!vault.sync_auto_reminders(now, "UTC", false).unwrap());
    assert_eq!(loan_text(&vault), stored);
    // And back.
    assert!(!vault.sync_auto_reminders(now, "UTC", true).unwrap());
    assert_eq!(loan_text(&vault), stored);
}

#[test]
fn changed_data_rewrites_the_text_in_the_callers_language() {
    let (_dir, mut vault) = empty_vault();
    let id = loan(&mut vault, 25);
    let now = secs_or_now(MARCH);
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    vault
        .doc
        .stores
        .iter_mut()
        .find(|store| store.id == id)
        .unwrap()
        .name = "Mortgage".into();
    assert!(vault.sync_auto_reminders(now, "UTC", false).unwrap());
    let text = loan_text(&vault);
    assert!(
        text.contains("Mortgage") && !text.contains("Ипотека"),
        "{text}"
    );
}

#[test]
fn a_reminder_is_read_in_the_language_of_the_reader() {
    let (_dir, mut vault) = empty_vault();
    loan(&mut vault, 25);
    let now = secs_or_now(MARCH);
    vault.sync_auto_reminders(now, "UTC", true).unwrap();
    let own = vault
        .create_reminder("Позвонить", "2026-03-12T09:00:00Z", None)
        .unwrap();
    let stored = loan_text(&vault);
    let english = vault.reminders_in(now, "UTC", false);
    let loan_reminder = english
        .iter()
        .find(|reminder| reminder.source.is_some())
        .unwrap();
    assert!(
        loan_reminder.text.contains("Ипотека"),
        "the name is the user's"
    );
    assert_ne!(loan_reminder.text, stored, "{}", loan_reminder.text);
    // The reader's own language gives the stored text; a typed text and a
    // text that matches no rendering are shown as they are.
    let russian = vault.reminders_in(now, "UTC", true);
    assert_eq!(
        russian
            .iter()
            .find(|reminder| reminder.source.is_some())
            .unwrap()
            .text,
        stored
    );
    assert_eq!(
        english
            .iter()
            .find(|reminder| reminder.id == own.id)
            .unwrap()
            .text,
        "Позвонить"
    );
    vault
        .doc
        .reminders
        .iter_mut()
        .find(|reminder| reminder.source.is_some())
        .unwrap()
        .text = "My own words".into();
    let edited = vault.reminders_in(now, "UTC", false);
    assert!(
        edited
            .iter()
            .any(|reminder| reminder.text == "My own words")
    );
    // Nothing was written.
    assert!(
        vault
            .doc
            .reminders
            .iter()
            .any(|reminder| reminder.text == "My own words")
    );
}

#[test]
fn a_notice_of_an_automatic_reminder_uses_the_language_of_the_device() {
    let (_dir, mut vault) = empty_vault();
    loan(&mut vault, 25);
    vault
        .sync_auto_reminders(secs_or_now(MARCH), "UTC", true)
        .unwrap();
    let notices = vault
        .due_notices(secs_or_now("2026-03-24T10:00:30Z"), "UTC", false)
        .unwrap();
    let body = &notices[0].body;
    assert_ne!(body, &loan_text(&vault));
    assert!(body.contains("Ипотека"), "{body}");
}
