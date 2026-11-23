//! Tests of the utilities area: the old inline tests (names and bodies
//! unchanged, but for the calls of moved helpers), then the pure functions on
//! a bare document and the delegate pins.

use crate::Amount;
use crate::calendar::now_secs;
use crate::error::Result;
use crate::household::utilities::*;
use crate::model::VaultDoc;
use crate::model::planning::{MeterReading, UtilityItem, UtilityKind};
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::Transaction;
use crate::service::Session;
use crate::service::test_support::*;
use pretty_assertions::assert_eq;

fn metered(vault: &mut Session) -> UtilityItem {
    metered_in(vault, "Flat")
}

fn metered_in(vault: &mut Session, flat: &str) -> UtilityItem {
    let flat = store(vault, flat, StoreKind::Property, None);
    crate::usecases::household::utilities::create_utility_item(
        vault,
        UtilityDraft {
            name: "Electricity".into(),
            active: true,
            kind: UtilityKind::Metered {
                unit: "kWh".into(),
                tariff: amount("5"),
                formula: None,
                zones: Vec::new(),
            },
            currency: "rub".into(),
            property_id: flat,
            store_id: None,
            category: "utilities".into(),
        },
    )
    .unwrap()
}

#[test]
fn a_meter_bills_consumption_times_tariff() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("1000"),
        "2026-01",
        None,
        None,
    )
    .unwrap();
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("1150"),
        "2026-02",
        None,
        None,
    )
    .unwrap();
    let draft = crate::usecases::household::utilities::utility_payment_draft(
        &vault, &item.id, "2026-02", "UTC",
    )
    .unwrap();
    assert_eq!(draft.consumption, Some(amount("150")));
    let Transaction::Payment(payment) = &draft.transaction else {
        panic!()
    };
    assert_eq!(payment.amount, amount("750"));
    assert_eq!(payment.meta.categories, vec!["utilities"]);
    // A meter only counts up.
    assert!(
        crate::usecases::household::utilities::add_meter_reading(
            &mut vault,
            &item.id,
            amount("900"),
            "2026-03",
            None,
            None
        )
        .is_err()
    );
    let trend = crate::household::utilities::meter_trend(&vault.books(), &item.id, "UTC").unwrap();
    assert_eq!(trend.months[1].amount, amount("750"));
}

#[test]
fn a_month_without_a_reading_bills_the_last_unpaid_one() {
    let (_dir, mut vault) = empty_vault();
    let (flat, item, _internet, card) = flat_with_bills(&mut vault);
    // Nothing read yet: an empty draft, not an error.
    let empty = crate::usecases::household::utilities::utility_payment_draft(
        &vault, &item.id, "2026-03", "UTC",
    )
    .unwrap();
    assert_eq!(empty.consumption, Some(Amount::ZERO));
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("1000"),
        "2026-01",
        None,
        None,
    )
    .unwrap();
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("1150"),
        "2026-02",
        None,
        None,
    )
    .unwrap();
    // Read in February, paid in March.
    let draft = crate::usecases::household::utilities::utility_payment_draft(
        &vault, &item.id, "2026-03", "UTC",
    )
    .unwrap();
    assert_eq!(draft.consumption, Some(amount("150")));
    // The month's bill picks up the same readings.
    let bill = utility_bill(&vault.books(), &flat, "2026-03", "UTC", now_secs()).unwrap();
    let line = bill
        .lines
        .iter()
        .find(|line| line.item.id == item.id)
        .unwrap();
    assert_eq!(line.previous_reading, Some(amount("1000")));
    assert_eq!(line.reading, Some(amount("1150")));
    assert_eq!(line.amount, Some(amount("750")));
    let mut paid = draft.transaction;
    if let Transaction::Payment(payment) = &mut paid {
        payment.store_id = card;
    }
    paid.meta_mut().timestamp = "2026-02-20T12:00:00Z".into();
    crate::usecases::money::transactions::add_transaction(&mut vault, paid).unwrap();
    // February is paid: March has nothing left to bill.
    let after = crate::usecases::household::utilities::utility_payment_draft(
        &vault, &item.id, "2026-03", "UTC",
    )
    .unwrap();
    assert_eq!(after.consumption, Some(Amount::ZERO));
}

#[test]
fn the_first_rent_payment_carries_the_deposit() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let mut seed = income_of(
        &card,
        AssetRef::fiat("RUB", "RUB"),
        "200000",
        "0.01",
        "salary",
    );
    seed.meta_mut().timestamp = "2026-01-01T00:00:00Z".into();
    crate::usecases::money::transactions::add_transaction(&mut vault, seed).unwrap();
    let flat = store(&mut vault, "Flat", StoreKind::Property, None);
    let item = crate::usecases::household::utilities::create_utility_item(
        &mut vault,
        UtilityDraft {
            name: "Rent".into(),
            active: true,
            kind: UtilityKind::Housing {
                amount: amount("40000"),
                deposit: Some(amount("40000")),
                due_day: 5,
                automation_id: None,
            },
            currency: "rub".into(),
            property_id: flat,
            store_id: Some(card.clone()),
            category: "rent_mortgage".into(),
        },
    )
    .unwrap();
    let first = crate::usecases::household::utilities::utility_payment_draft(
        &vault, &item.id, "2026-02", "UTC",
    )
    .unwrap();
    assert!(first.includes_deposit);
    let Transaction::Payment(payment) = first.transaction.clone() else {
        panic!()
    };
    assert_eq!(payment.amount, amount("80000"));
    crate::usecases::money::transactions::add_transaction(&mut vault, first.transaction).unwrap();
    let second = crate::usecases::household::utilities::utility_payment_draft(
        &vault, &item.id, "2026-03", "UTC",
    )
    .unwrap();
    assert!(!second.includes_deposit);
    let Transaction::Payment(payment) = second.transaction else {
        panic!()
    };
    assert_eq!(payment.amount, amount("40000"));
}

#[test]
fn the_trend_forecasts_and_flags_a_spike() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    // Consumption 100, 110, 120, 130 → bills 500, 550, 600, 650, then 2000.
    for (period, value) in [
        ("2026-01", "0"),
        ("2026-02", "100"),
        ("2026-03", "210"),
        ("2026-04", "330"),
        ("2026-05", "460"),
    ] {
        crate::usecases::household::utilities::add_meter_reading(
            &mut vault,
            &item.id,
            amount(value),
            period,
            None,
            None,
        )
        .unwrap();
    }
    let steady = crate::household::utilities::meter_trend(&vault.books(), &item.id, "UTC").unwrap();
    assert_eq!(steady.forecast, Some(amount("700")));
    assert!(!steady.anomaly);
    assert_eq!(steady.delta_pct, Some(amount("8.3")));
    assert_eq!(steady.average, Some(amount("575")));

    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("860"),
        "2026-06",
        None,
        None,
    )
    .unwrap();
    let spike = crate::household::utilities::meter_trend(&vault.books(), &item.id, "UTC").unwrap();
    assert!(spike.anomaly);
}

#[test]
fn a_utility_needs_a_property_and_the_summary_adds_it_up() {
    let (_dir, mut vault) = empty_vault();
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    let draft = |property_id: &str| UtilityDraft {
        name: "Internet".into(),
        active: true,
        kind: UtilityKind::Static {
            amount: amount("30"),
        },
        currency: "usd".into(),
        property_id: property_id.into(),
        store_id: None,
        category: "utilities".into(),
    };
    assert!(
        crate::usecases::household::utilities::create_utility_item(&mut vault, draft(&bank))
            .is_err()
    );
    let flat = store(&mut vault, "Flat", StoreKind::Property, None);
    crate::usecases::household::utilities::create_utility_item(&mut vault, draft(&flat)).unwrap();
    let summary =
        crate::usecases::household::utilities::property_summary(&vault, &flat, "UTC").unwrap();
    assert_eq!(summary.utility_count, 1);
    assert_eq!(summary.utilities_month_base, amount("30"));
    assert_eq!(summary.monthly_cost_base, amount("30"));
}

/// A funded card and a flat with a meter and a fixed bill.
fn flat_with_bills(vault: &mut Session) -> (String, UtilityItem, UtilityItem, String) {
    let card = store(vault, "Card", StoreKind::Bank, None);
    let mut seed = income_of(
        &card,
        AssetRef::fiat("RUB", "RUB"),
        "100000",
        "0.01",
        "salary",
    );
    seed.meta_mut().timestamp = "2026-01-01T00:00:00Z".into();
    crate::usecases::money::transactions::add_transaction(vault, seed).unwrap();
    let meter = metered(vault);
    let flat = meter.property_id.clone();
    let internet = crate::usecases::household::utilities::create_utility_item(
        vault,
        UtilityDraft {
            name: "Internet".into(),
            active: true,
            kind: UtilityKind::Static {
                amount: amount("700"),
            },
            currency: "rub".into(),
            property_id: flat.clone(),
            store_id: None,
            category: "utilities".into(),
        },
    )
    .unwrap();
    (flat, meter, internet, card)
}

#[test]
fn the_bill_estimates_an_unread_meter_and_compares_with_its_average() {
    let (_dir, mut vault) = empty_vault();
    let (flat, meter, _internet, card) = flat_with_bills(&mut vault);
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &meter.id,
        amount("1000"),
        "2026-01",
        None,
        None,
    )
    .unwrap();
    // February and March read and paid on the bill, 500 each.
    for (reading, period) in [("1100", "2026-02"), ("1200", "2026-03")] {
        crate::usecases::household::utilities::pay_utility_bill(
            &mut vault,
            PayUtilityBillInput {
                property_id: flat.clone(),
                period: period.into(),
                store_id: card.clone(),
                timestamp: Some(format!("{period}-10T12:00:00Z")),
                lines: vec![PayUtilityLine {
                    item_id: meter.id.clone(),
                    amount: amount("500"),
                    reading: Some(amount(reading)),
                    reading_day: None,
                    zone_readings: Vec::new(),
                }],
            },
            "UTC",
        )
        .unwrap();
    }
    // April not read yet: the meter is estimated, the internet is fixed.
    let bill = utility_bill(&vault.books(), &flat, "2026-04", "UTC", now_secs()).unwrap();
    let line = bill
        .lines
        .iter()
        .find(|line| line.item.id == meter.id)
        .unwrap();
    assert_eq!(line.amount, None);
    assert_eq!(line.estimate, Some(amount("500")));
    assert_eq!(bill.unread, 1);
    assert!(bill.estimated);
    assert_eq!(bill.total, amount("1200"));
    // February and March billed 500 each by the meter.
    assert_eq!(bill.average, Some(amount("500")));
    assert_eq!(bill.delta_pct, Some(amount("140")));
}

#[test]
fn a_whole_bill_is_paid_at_once_and_linked_to_its_items() {
    let (_dir, mut vault) = empty_vault();
    let (flat, meter, internet, card) = flat_with_bills(&mut vault);
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &meter.id,
        amount("1000"),
        "2026-01",
        None,
        None,
    )
    .unwrap();

    let bill = utility_bill(&vault.books(), &flat, "2026-02", "UTC", now_secs()).unwrap();
    assert_eq!(bill.lines.len(), 2);
    let meter_line = bill
        .lines
        .iter()
        .find(|line| line.item.id == meter.id)
        .unwrap();
    assert_eq!(meter_line.previous_reading, Some(amount("1000")));
    assert_eq!(meter_line.amount, None, "no reading for February yet");

    let paid = crate::usecases::household::utilities::pay_utility_bill(
        &mut vault,
        PayUtilityBillInput {
            property_id: flat.clone(),
            period: "2026-02".into(),
            store_id: card,
            timestamp: Some("2026-02-10T12:00:00Z".into()),
            lines: vec![
                PayUtilityLine {
                    item_id: meter.id.clone(),
                    amount: amount("500"),
                    reading: Some(amount("1100")),
                    reading_day: None,
                    zone_readings: Vec::new(),
                },
                PayUtilityLine {
                    item_id: internet.id.clone(),
                    amount: amount("700"),
                    reading: None,
                    reading_day: None,
                    zone_readings: Vec::new(),
                },
            ],
        },
        "UTC",
    )
    .unwrap();
    assert_eq!(paid.len(), 2);
    assert!(paid.iter().all(|tx| tx.meta().utility_item_id.is_some()));
    // The comment no longer has to match: the link counts.
    let after = utility_bill(&vault.books(), &flat, "2026-02", "UTC", now_secs()).unwrap();
    let internet_line = after
        .lines
        .iter()
        .find(|line| line.item.id == internet.id)
        .unwrap();
    assert_eq!(internet_line.paid, amount("700"));
    let meter_line = after
        .lines
        .iter()
        .find(|line| line.item.id == meter.id)
        .unwrap();
    assert_eq!(meter_line.reading, Some(amount("1100")));
}

#[test]
fn a_failing_line_leaves_the_vault_untouched() {
    let (_dir, mut vault) = empty_vault();
    let (flat, meter, internet, card) = flat_with_bills(&mut vault);
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &meter.id,
        amount("1000"),
        "2026-01",
        None,
        None,
    )
    .unwrap();
    let before = vault.doc.transactions.len();
    let result = crate::usecases::household::utilities::pay_utility_bill(
        &mut vault,
        PayUtilityBillInput {
            property_id: flat,
            period: "2026-02".into(),
            store_id: card,
            timestamp: None,
            lines: vec![
                PayUtilityLine {
                    item_id: internet.id,
                    amount: amount("700"),
                    reading: None,
                    reading_day: None,
                    zone_readings: Vec::new(),
                },
                // Below last month's reading: refused.
                PayUtilityLine {
                    item_id: meter.id.clone(),
                    amount: amount("1"),
                    reading: Some(amount("900")),
                    reading_day: None,
                    zone_readings: Vec::new(),
                },
            ],
        },
        "UTC",
    );
    assert!(result.is_err());
    assert_eq!(vault.doc.transactions.len(), before);
    assert_eq!(vault.doc.meter_readings.len(), 1);
}

#[test]
fn a_reading_can_be_corrected_within_the_meter_rules() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("1000"),
        "2026-01",
        None,
        None,
    )
    .unwrap();
    let feb = crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("1200"),
        "2026-02",
        None,
        None,
    )
    .unwrap();
    let fixed = crate::usecases::household::utilities::update_meter_reading(
        &mut vault,
        &feb.id,
        amount("1150"),
        "2026-02",
        None,
    )
    .unwrap();
    assert_eq!(fixed.id, feb.id);
    assert_eq!(
        monthly_readings(&vault.doc, &item.id)["2026-02"].end,
        amount("1150")
    );
    assert!(
        crate::usecases::household::utilities::update_meter_reading(
            &mut vault,
            &feb.id,
            amount("900"),
            "2026-02",
            None
        )
        .is_err()
    );
    assert_eq!(
        monthly_readings(&vault.doc, &item.id)["2026-02"].end,
        amount("1150"),
        "a refused fix keeps the old value"
    );
}

/// Moved on the 15th: the old flat is read that day, the new flat's meter
/// starts that day; each month bill runs from its own start.
#[test]
fn a_move_mid_month_bills_each_flat_from_its_own_readings() {
    let (_dir, mut vault) = empty_vault();
    let old_flat = metered_in(&mut vault, "Old flat");
    let new_flat = metered_in(&mut vault, "New flat");
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &old_flat.id,
        amount("100"),
        "2026-09",
        None,
        None,
    )
    .unwrap();
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &old_flat.id,
        amount("130"),
        "2026-10",
        Some(15),
        None,
    )
    .unwrap();
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &new_flat.id,
        amount("500"),
        "2026-10",
        Some(15),
        None,
    )
    .unwrap();

    // Before the month ends the new flat has only its start: nothing used yet.
    let line = |vault: &Session, item: &UtilityItem| {
        let property = item.property_id.clone();
        utility_bill(&vault.books(), &property, "2026-10", "UTC", now_secs())
            .unwrap()
            .lines
            .into_iter()
            .find(|line| line.item.id == item.id)
            .unwrap()
    };
    let started = line(&vault, &new_flat);
    assert_eq!(started.previous_reading, Some(amount("500")));
    assert_eq!(started.reading, None);

    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &new_flat.id,
        amount("520"),
        "2026-10",
        None,
        None,
    )
    .unwrap();
    let old_bill = line(&vault, &old_flat);
    assert_eq!(old_bill.previous_reading, Some(amount("100")));
    assert_eq!(old_bill.consumption, Some(amount("30")));
    let new_bill = line(&vault, &new_flat);
    assert_eq!(new_bill.previous_reading, Some(amount("500")));
    assert_eq!(new_bill.consumption, Some(amount("20")));
    assert_eq!(new_bill.amount, Some(amount("100")));

    let trend =
        crate::household::utilities::meter_trend(&vault.books(), &new_flat.id, "UTC").unwrap();
    assert_eq!(trend.months.len(), 1);
    assert_eq!(trend.months[0].readings.len(), 2);
    assert_eq!(trend.months[0].consumption, Some(amount("20")));
}

#[test]
fn readings_of_one_month_keep_their_order_by_day() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("100"),
        "2026-10",
        Some(10),
        None,
    )
    .unwrap();
    assert!(
        crate::usecases::household::utilities::add_meter_reading(
            &mut vault,
            &item.id,
            amount("90"),
            "2026-10",
            Some(20),
            None
        )
        .is_err(),
        "the 20th cannot be below the 10th"
    );
    assert!(
        crate::usecases::household::utilities::add_meter_reading(
            &mut vault,
            &item.id,
            amount("110"),
            "2026-10",
            Some(5),
            None
        )
        .is_err(),
        "the 5th cannot be above the 10th"
    );
    assert!(
        crate::usecases::household::utilities::add_meter_reading(
            &mut vault,
            &item.id,
            amount("110"),
            "2026-02",
            Some(30),
            None
        )
        .is_err(),
        "February has no 30th"
    );
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("120"),
        "2026-10",
        Some(20),
        None,
    )
    .unwrap();
    assert_eq!(readings_of(&vault.doc.meter_readings, &item.id).len(), 2);
}

#[test]
fn a_reading_without_a_day_does_not_block_a_later_one() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    // Entered on the 5th without a day, as the old form did.
    vault.doc.meter_readings.push(MeterReading {
        id: "old".into(),
        item_id: item.id.clone(),
        value: amount("2531.5"),
        period: "2026-10".into(),
        recorded_at: "2026-10-05T09:00:00Z".into(),
        day: None,
        zone: None,
    });
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("2540"),
        "2026-10",
        Some(12),
        None,
    )
    .unwrap();
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("2552"),
        "2026-10",
        Some(20),
        None,
    )
    .unwrap();
    assert!(
        crate::usecases::household::utilities::add_meter_reading(
            &mut vault,
            &item.id,
            amount("2530"),
            "2026-10",
            Some(8),
            None
        )
        .is_err(),
        "the 8th still cannot be below the 5th"
    );
    let values: Vec<String> = readings_of(&vault.doc.meter_readings, &item.id)
        .iter()
        .map(|reading| reading.value.to_string())
        .collect();
    assert_eq!(values, ["2531.5", "2540", "2552"]);
}

#[test]
fn a_second_reading_the_same_day_corrects_the_first() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("100"),
        "2026-10",
        Some(12),
        None,
    )
    .unwrap();
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("101"),
        "2026-10",
        Some(12),
        None,
    )
    .unwrap();
    let readings = readings_of(&vault.doc.meter_readings, &item.id);
    assert_eq!(readings.len(), 1);
    assert_eq!(readings[0].value, amount("101"));
}

fn meter_with(vault: &mut Session, formula: &str, zones: &[&str]) -> Result<UtilityItem> {
    let name = format!("Dacha {}", vault.doc.stores.len());
    let flat = store(vault, &name, StoreKind::Property, None);
    crate::usecases::household::utilities::create_utility_item(
        vault,
        UtilityDraft {
            name: "Свет".into(),
            active: true,
            kind: UtilityKind::Metered {
                unit: "kWh".into(),
                tariff: amount("0"),
                formula: Some(formula.into()),
                zones: zones.iter().map(|zone| (*zone).to_string()).collect(),
            },
            currency: "rub".into(),
            property_id: flat,
            store_id: None,
            category: "utilities".into(),
        },
    )
}

fn draft_amount(vault: &Session, item: &UtilityItem, period: &str) -> Amount {
    let draft = crate::usecases::household::utilities::utility_payment_draft(
        vault, &item.id, period, "UTC",
    )
    .unwrap();
    let Transaction::Payment(payment) = &draft.transaction else {
        panic!()
    };
    payment.amount
}

#[test]
fn a_meter_bills_by_its_own_formula() {
    let (_dir, mut vault) = empty_vault();
    let item = meter_with(&mut vault, " (показание - предыдущее) * 3.5 * 10 ", &[]).unwrap();
    let UtilityKind::Metered { formula, .. } = &item.kind else {
        panic!()
    };
    assert_eq!(
        formula.as_deref(),
        Some("(показание - предыдущее) * 3.5 * 10")
    );
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("1200"),
        "2026-01",
        None,
        None,
    )
    .unwrap();
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("1345"),
        "2026-02",
        None,
        None,
    )
    .unwrap();
    assert_eq!(draft_amount(&vault, &item, "2026-02"), amount("5075"));
    let trend = crate::household::utilities::meter_trend(&vault.books(), &item.id, "UTC").unwrap();
    assert_eq!(trend.months[1].amount, amount("5075"));
}

#[test]
fn a_wrong_formula_is_not_saved() {
    let (_dir, mut vault) = empty_vault();
    let err = meter_with(&mut vault, "расход * х", &[]).unwrap_err();
    assert!(err.to_string().contains("character 10"), "{err}");
    assert!(meter_with(&mut vault, "расход_вечер", &["день", "ночь"]).is_err());
    assert!(meter_with(&mut vault, "расход", &["день", " День "]).is_err());
    // Without a formula a meter still needs a tariff.
    let flat = store(&mut vault, "Flat", StoreKind::Property, None);
    let no_tariff = crate::usecases::household::utilities::create_utility_item(
        &mut vault,
        UtilityDraft {
            name: "Water".into(),
            active: true,
            kind: UtilityKind::Metered {
                unit: "m3".into(),
                tariff: amount("0"),
                formula: Some("  ".into()),
                zones: Vec::new(),
            },
            currency: "rub".into(),
            property_id: flat,
            store_id: None,
            category: "utilities".into(),
        },
    );
    assert!(no_tariff.is_err());
}

#[test]
fn a_two_rate_meter_reads_and_bills_per_zone() {
    let (_dir, mut vault) = empty_vault();
    let item = meter_with(
        &mut vault,
        "расход_день * 6 + расход_ночь * 3 + 100",
        &["День", "Ночь"],
    )
    .unwrap();
    // A zone must be named, and be one of the meter's.
    assert!(
        crate::usecases::household::utilities::add_meter_reading(
            &mut vault,
            &item.id,
            amount("600"),
            "2026-01",
            None,
            None
        )
        .is_err()
    );
    assert!(
        crate::usecases::household::utilities::add_meter_reading(
            &mut vault,
            &item.id,
            amount("600"),
            "2026-01",
            None,
            Some("вечер")
        )
        .is_err()
    );
    for (value, period, zone) in [
        ("600", "2026-01", "день"),
        ("400", "2026-01", "ночь"),
        ("800", "2026-02", "день"),
        ("500", "2026-02", "ночь"),
    ] {
        crate::usecases::household::utilities::add_meter_reading(
            &mut vault,
            &item.id,
            amount(value),
            period,
            None,
            Some(zone),
        )
        .unwrap();
    }
    // Each zone counts up on its own: night below day is fine.
    assert_eq!(draft_amount(&vault, &item, "2026-02"), amount("1600"));
    let trend = crate::household::utilities::meter_trend(&vault.books(), &item.id, "UTC").unwrap();
    let february = &trend.months[1];
    assert_eq!(february.consumption, Some(amount("300")));
    assert_eq!(february.zones.len(), 2);
    assert_eq!(february.zones[0].zone, "День");
    assert_eq!(february.zones[0].consumption, Some(amount("200")));

    // February is unpaid: March's bill offers it, zone by zone.
    let bill = utility_bill(
        &vault.books(),
        &item.property_id,
        "2026-03",
        "UTC",
        now_secs(),
    )
    .unwrap();
    let line = &bill.lines[0];
    assert_eq!(line.amount, Some(amount("1600")));
    let zones: Vec<_> = line
        .zones
        .iter()
        .map(|zone| (zone.previous_reading, zone.reading))
        .collect();
    assert_eq!(
        zones,
        vec![
            (Some(amount("600")), Some(amount("800"))),
            (Some(amount("400")), Some(amount("500"))),
        ]
    );
    let card = store(&mut vault, "Card", StoreKind::Cash, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&card, "5000", "opening"),
    )
    .unwrap();
    crate::usecases::household::utilities::pay_utility_bill(
        &mut vault,
        PayUtilityBillInput {
            property_id: item.property_id.clone(),
            period: "2026-03".into(),
            store_id: card,
            timestamp: Some("2026-03-10T12:00:00Z".into()),
            lines: vec![PayUtilityLine {
                item_id: item.id.clone(),
                amount: amount("1000"),
                reading: None,
                reading_day: None,
                zone_readings: vec![
                    ZoneReadingInput {
                        zone: "день".into(),
                        reading: amount("900"),
                    },
                    ZoneReadingInput {
                        zone: "ночь".into(),
                        reading: amount("550"),
                    },
                ],
            }],
        },
        "UTC",
    )
    .unwrap();
    let trend = crate::household::utilities::meter_trend(&vault.books(), &item.id, "UTC").unwrap();
    // 100 × 6 + 50 × 3 + 100.
    assert_eq!(trend.months[2].amount, amount("850"));
}

#[test]
fn the_preview_prices_the_typed_readings() {
    let zoned = preview_utility_formula(&UtilityFormulaInput {
        tariff: amount("5"),
        formula: Some("расход_день * тариф + расход_ночь * 2".into()),
        zones: vec!["день".into(), "ночь".into()],
        readings: vec![
            FormulaZoneReading {
                zone: Some("день".into()),
                previous: amount("10"),
                current: amount("20"),
            },
            FormulaZoneReading {
                zone: Some("ночь".into()),
                previous: amount("5"),
                current: amount("10"),
            },
        ],
        days: None,
    });
    assert_eq!(zoned.amount, Some(amount("60")));
    assert_eq!(zoned.consumption, Some(amount("15")));
    let plain = preview_utility_formula(&UtilityFormulaInput {
        tariff: amount("5"),
        formula: None,
        zones: Vec::new(),
        readings: vec![FormulaZoneReading {
            zone: None,
            previous: amount("100"),
            current: amount("130"),
        }],
        days: None,
    });
    assert_eq!(plain.amount, Some(amount("150")));
    let broken = preview_utility_formula(&UtilityFormulaInput {
        formula: Some("расход / 0".into()),
        ..plain_input()
    });
    assert!(broken.amount.is_none() && broken.issue.is_some());
}

fn plain_input() -> UtilityFormulaInput {
    UtilityFormulaInput {
        tariff: amount("5"),
        formula: None,
        zones: Vec::new(),
        readings: Vec::new(),
        days: None,
    }
}

#[test]
fn a_meter_from_an_older_app_still_reads() {
    let kind = serde_json::json!({ "metered": { "unit": "kWh", "tariff": "5" } });
    let reading = serde_json::json!({
        "id": "r", "item_id": "i", "value": "10", "period": "2026-01",
        "recorded_at": "2026-01-31T00:00:00Z",
    });
    let from_json: UtilityKind = serde_json::from_value(kind.clone()).unwrap();
    assert_eq!(
        from_json,
        UtilityKind::Metered {
            unit: "kWh".into(),
            tariff: amount("5"),
            formula: None,
            zones: Vec::new(),
        }
    );
    let packed = rmp_serde::to_vec_named(&kind).unwrap();
    assert_eq!(
        rmp_serde::from_slice::<UtilityKind>(&packed).unwrap(),
        from_json
    );
    let old: MeterReading = serde_json::from_value(reading.clone()).unwrap();
    assert_eq!(old.zone, None);
    let packed = rmp_serde::to_vec_named(&reading).unwrap();
    assert_eq!(rmp_serde::from_slice::<MeterReading>(&packed).unwrap(), old);
}

#[test]
fn a_reading_below_the_previous_one_is_flagged_not_rewritten() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("100"),
        "2026-09",
        Some(10),
        None,
    )
    .unwrap();
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("120"),
        "2026-10",
        Some(10),
        None,
    )
    .unwrap();
    let clean = crate::household::utilities::meter_trend(&vault.books(), &item.id, "UTC").unwrap();
    assert!(
        clean
            .months
            .iter()
            .all(|month| month.out_of_order.is_empty())
    );

    // What a merge of two devices can leave: a lower reading on a later day.
    vault.doc.meter_readings.push(MeterReading {
        id: "late".into(),
        item_id: item.id.clone(),
        value: amount("90"),
        period: "2026-10".into(),
        recorded_at: "2026-10-20T00:00:00Z".into(),
        day: Some(20),
        zone: None,
    });
    let trend = crate::household::utilities::meter_trend(&vault.books(), &item.id, "UTC").unwrap();
    let october = trend
        .months
        .iter()
        .find(|month| month.period == "2026-10")
        .unwrap();
    assert_eq!(october.out_of_order, vec!["late".to_string()]);
    let september = trend
        .months
        .iter()
        .find(|month| month.period == "2026-09")
        .unwrap();
    assert!(september.out_of_order.is_empty());
    assert!(
        vault
            .doc
            .meter_readings
            .iter()
            .any(|reading| reading.value == amount("90")),
        "values are not rewritten"
    );
}

#[test]
fn zones_are_compared_apart() {
    let reading = |id: &str, zone: &str, value: &str, day: u8| MeterReading {
        id: id.into(),
        item_id: "item".into(),
        value: amount(value),
        period: "2026-10".into(),
        recorded_at: String::new(),
        day: Some(day),
        zone: Some(zone.into()),
    };
    let readings = vec![
        reading("day-1", "day", "500", 1),
        reading("night-1", "night", "100", 2),
        reading("day-2", "day", "510", 3),
        reading("night-2", "night", "90", 4),
    ];
    assert_eq!(out_of_order_ids(&readings), vec!["night-2".to_string()]);
}

fn draft_for(property_id: &str, name: &str) -> UtilityDraft {
    UtilityDraft {
        name: name.into(),
        active: true,
        kind: UtilityKind::Static {
            amount: amount("10"),
        },
        currency: "RUB".into(),
        property_id: property_id.into(),
        store_id: None,
        category: "utilities".into(),
    }
}

#[test]
fn the_items_are_listed_by_name_on_a_bare_document() {
    let mut doc = VaultDoc::default();
    for name in ["water", "Gas", "electricity"] {
        let draft = draft_for("flat", name);
        doc.utility_items.push(UtilityItem {
            id: name.into(),
            name: draft.name,
            active: true,
            kind: draft.kind,
            currency: "rub".into(),
            property_id: "flat".into(),
            store_id: None,
            category: "utilities".into(),
        });
    }
    let names: Vec<String> = utility_items(&doc)
        .into_iter()
        .map(|item| item.name)
        .collect();
    assert_eq!(names, ["electricity", "Gas", "water"]);
    assert!(utility_item(&doc, "gone").is_err());
}

#[test]
fn a_bare_document_refuses_what_has_no_item_or_reading() {
    let doc = VaultDoc::default();
    assert!(plan_delete_utility(&doc, "gone").is_err());
    assert!(require_reading(&doc, "gone").is_err());
    let input = ReadingInput {
        item_id: "gone",
        value: amount("1"),
        period: "2026-01",
        day: None,
        zone: None,
    };
    assert!(
        plan_add_reading(
            &doc,
            input,
            &|_, _, _| String::new(),
            "2026-01-01T00:00:00Z"
        )
        .is_err()
    );
}

#[test]
fn a_housing_item_needs_its_automation_to_exist() {
    let (_dir, mut vault) = empty_vault();
    let flat = store(&mut vault, "Flat", StoreKind::Property, None);
    let mut draft = draft_for(&flat, "Rent");
    draft.kind = UtilityKind::Housing {
        amount: amount("500"),
        deposit: None,
        due_day: 5,
        automation_id: Some("auto-1".into()),
    };
    assert!(plan_create_utility(&vault.doc, draft.clone(), &|_| false).is_err());
    assert!(plan_create_utility(&vault.doc, draft, &|id| id == "auto-1").is_ok());
}

#[test]
fn the_delegates_equal_the_scenarios() {
    use crate::usecases::household::utilities as scenario;
    let (_dir_a, mut through_delegate) = empty_vault();
    let (_dir_b, mut through_scenario) = empty_vault();
    let flat_a = store(&mut through_delegate, "Flat", StoreKind::Property, None);
    let flat_b = store(&mut through_scenario, "Flat", StoreKind::Property, None);
    let item_a = crate::usecases::household::utilities::create_utility_item(
        &mut through_delegate,
        draft_for(&flat_a, " Internet "),
    )
    .unwrap();
    let item_b =
        scenario::create_utility_item(&mut through_scenario, draft_for(&flat_b, " Internet "))
            .unwrap();
    assert_eq!(item_a.name, "Internet");
    assert_eq!(item_a.currency, item_b.currency);
    assert_eq!(item_a.name, item_b.name);
    let renamed = draft_for(&flat_a, "Fibre");
    let updated = crate::usecases::household::utilities::update_utility_item(
        &mut through_delegate,
        &item_a.id,
        renamed,
    )
    .unwrap();
    assert_eq!(updated.id, item_a.id);
    assert_eq!(updated.name, "Fibre");
    assert_eq!(
        crate::household::utilities::utility_items(&through_delegate.doc),
        vec![updated]
    );
    crate::usecases::household::utilities::delete_utility_item(&mut through_delegate, &item_a.id)
        .unwrap();
    scenario::delete_utility_item(&mut through_scenario, &item_b.id).unwrap();
    assert!(through_delegate.doc.utility_items.is_empty());
    assert!(through_scenario.doc.utility_items.is_empty());
    assert!(
        crate::usecases::household::utilities::delete_utility_item(
            &mut through_delegate,
            &item_a.id
        )
        .is_err()
    );
}

#[test]
fn deleting_an_item_takes_its_readings_with_it() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("10"),
        "2026-01",
        None,
        None,
    )
    .unwrap();
    let kept = metered_in(&mut vault, "Dacha");
    crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &kept.id,
        amount("7"),
        "2026-01",
        None,
        None,
    )
    .unwrap();
    crate::usecases::household::utilities::delete_utility_item(&mut vault, &item.id).unwrap();
    assert_eq!(vault.doc.meter_readings.len(), 1);
    assert_eq!(vault.doc.meter_readings[0].item_id, kept.id);
}

#[test]
fn a_reading_is_added_corrected_and_deleted_through_the_delegates() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    let first = crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("10"),
        "2026-01",
        Some(5),
        None,
    )
    .unwrap();
    let again = crate::usecases::household::utilities::add_meter_reading(
        &mut vault,
        &item.id,
        amount("11"),
        "2026-01",
        Some(5),
        None,
    )
    .unwrap();
    assert_eq!(first.id, again.id, "the same day is the same record");
    assert_eq!(vault.doc.meter_readings.len(), 1);
    let moved = crate::usecases::household::utilities::update_meter_reading(
        &mut vault,
        &first.id,
        amount("12"),
        "2026-01",
        Some(9),
    )
    .unwrap();
    assert_ne!(moved.id, first.id, "the id follows the day");
    assert_eq!(vault.doc.meter_readings.len(), 1);
    assert!(
        crate::usecases::household::utilities::delete_meter_reading(&mut vault, &first.id).is_err()
    );
    crate::usecases::household::utilities::delete_meter_reading(&mut vault, &moved.id).unwrap();
    assert!(vault.doc.meter_readings.is_empty());
}

/// Counts the commits a vault marks once it batches its writes.
#[derive(Default)]
struct Commits(std::sync::atomic::AtomicUsize);

impl crate::kernel::session::SaveHook for Commits {
    fn dirty(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn paying_a_bill_is_one_commit() {
    let (_dir, mut vault) = empty_vault();
    let (flat, meter, _internet, card) = flat_with_bills(&mut vault);
    let commits = std::sync::Arc::new(Commits::default());
    vault.set_write_behind(Some(commits.clone()));
    let paid = crate::usecases::household::utilities::pay_utility_bill(
        &mut vault,
        PayUtilityBillInput {
            property_id: flat,
            period: "2026-03".into(),
            store_id: card,
            timestamp: Some("2026-03-10T12:00:00Z".into()),
            lines: vec![PayUtilityLine {
                item_id: meter.id.clone(),
                amount: amount("500"),
                reading: Some(amount("1200")),
                reading_day: None,
                zone_readings: Vec::new(),
            }],
        },
        "UTC",
    )
    .unwrap();
    assert_eq!(paid.len(), 1);
    assert_eq!(vault.doc.meter_readings.len(), 1);
    assert_eq!(
        commits.0.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the readings, the payments and the holdings are one write"
    );
}

#[test]
fn a_planned_reading_is_stamped_with_the_given_time() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    let input = ReadingInput {
        item_id: &item.id,
        value: amount("10"),
        period: "2026-01",
        day: None,
        zone: None,
    };
    let reading = plan_add_reading(
        &vault.doc,
        input,
        &|_, _, _| "reading".to_string(),
        "2001-02-03T04:05:06Z",
    )
    .unwrap();
    assert_eq!(reading.recorded_at, "2001-02-03T04:05:06Z");
}

#[test]
fn a_bill_is_due_on_the_given_day() {
    let (_dir, mut vault) = empty_vault();
    let item = metered(&mut vault);
    let now = crate::time::unix_secs("2026-03-17T08:00:00Z").unwrap();
    let bill = utility_bill(&vault.books(), &item.property_id, "2026-03", "UTC", now).unwrap();
    assert!(bill.due_at.starts_with("2026-03-17"), "{}", bill.due_at);
}
