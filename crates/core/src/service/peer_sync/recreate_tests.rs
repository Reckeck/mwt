//! A deterministic id means a deleted record and its later re-creation share
//! one id. The new record must not be swallowed by the old tombstone, and a
//! delete against a concurrent edit must leave the devices equal.

use super::convergence_catalog::add_catalog_stores;
use super::convergence_tests::Ledger;
use super::tests::{settle, two_devices};
use super::*;
use crate::model::store::{StoreKind, StoreScope};
use crate::service::contacts::DebtTermsDraft;
use crate::service::goals::GoalDraft;
use crate::service::test_support::{amount, income, store};
use crate::service::valuation::ValuationInput;

fn pair() -> (tempfile::TempDir, Vec<UnlockedVault>, Ledger) {
    let (dir, left, right) = two_devices();
    let mut devices = vec![left, right];
    let mut ledger = Ledger::default();
    add_catalog_stores(&mut devices, &mut ledger);
    (dir, devices, ledger)
}

fn meet(devices: &mut [UnlockedVault]) {
    let (left, right) = devices.split_at_mut(1);
    settle(&mut left[0], &mut right[0]);
}

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(3));
}

fn terms(contact: &str, note: &str) -> DebtTermsDraft {
    DebtTermsDraft {
        contact_id: contact.into(),
        asset: "USD".into(),
        due_at: None,
        note: note.into(),
    }
}

fn notes(vault: &UnlockedVault) -> Vec<String> {
    vault
        .doc
        .debt_terms
        .iter()
        .map(|row| row.note.clone())
        .collect()
}

#[test]
fn debt_terms_created_again_after_they_were_cleared_reach_the_other_device() {
    let (_dir, mut devices, ledger) = pair();
    let contact = ledger.anchors[0].clone();
    devices[0].set_debt_terms(terms(&contact, "first")).unwrap();
    meet(&mut devices);
    pause();
    devices[0].set_debt_terms(terms(&contact, "")).unwrap();
    assert!(devices[0].doc.debt_terms.is_empty());
    meet(&mut devices);
    assert!(devices[1].doc.debt_terms.is_empty());
    pause();
    devices[0]
        .set_debt_terms(terms(&contact, "second"))
        .unwrap();
    meet(&mut devices);
    assert_eq!(notes(&devices[1]), vec!["second"]);
    assert_eq!(notes(&devices[0]), vec!["second"]);
}

#[test]
fn debt_terms_cleared_on_one_device_and_edited_on_the_other_end_equal() {
    let (_dir, mut devices, ledger) = pair();
    let contact = ledger.anchors[0].clone();
    devices[0].set_debt_terms(terms(&contact, "first")).unwrap();
    meet(&mut devices);
    pause();
    devices[0].set_debt_terms(terms(&contact, "")).unwrap();
    pause();
    devices[1]
        .set_debt_terms(terms(&contact, "edited"))
        .unwrap();
    meet(&mut devices);
    assert_eq!(notes(&devices[0]), notes(&devices[1]));
}

#[test]
fn a_reading_made_again_after_it_was_deleted_reaches_the_other_device() {
    let (_dir, mut devices, ledger) = pair();
    let item = ledger.utility_id.clone();
    let id = devices[0]
        .add_meter_reading(&item, amount("100"), "2026-03", Some(2), None)
        .unwrap()
        .id;
    meet(&mut devices);
    pause();
    devices[0].delete_meter_reading(&id).unwrap();
    meet(&mut devices);
    assert!(devices[1].doc.meter_readings.is_empty());
    pause();
    let again = devices[0]
        .add_meter_reading(&item, amount("130"), "2026-03", Some(2), None)
        .unwrap();
    assert_eq!(again.id, id, "the same day names the same record");
    meet(&mut devices);
    for device in &devices {
        assert_eq!(device.doc.meter_readings.len(), 1);
        assert_eq!(device.doc.meter_readings[0].value, amount("130"));
    }
}

#[test]
fn a_reading_deleted_on_one_device_and_corrected_on_the_other_end_equal() {
    let (_dir, mut devices, ledger) = pair();
    let item = ledger.utility_id.clone();
    let id = devices[0]
        .add_meter_reading(&item, amount("100"), "2026-03", Some(2), None)
        .unwrap()
        .id;
    meet(&mut devices);
    pause();
    devices[0].delete_meter_reading(&id).unwrap();
    pause();
    devices[1]
        .update_meter_reading(&id, amount("110"), "2026-03", Some(2))
        .unwrap();
    meet(&mut devices);
    assert_eq!(devices[0].doc.meter_readings, devices[1].doc.meter_readings);
}

fn valuation(ledger: &Ledger, price: &str) -> ValuationInput {
    ValuationInput {
        store_id: ledger.property_id.clone(),
        asset_key: ledger.thing_key.clone(),
        date: "2026-03-01".into(),
        unit_price: amount(price),
        currency: "usd".into(),
    }
}

#[test]
fn a_valuation_made_again_after_it_was_deleted_reaches_the_other_device() {
    let (_dir, mut devices, ledger) = pair();
    let id = devices[0]
        .add_valuation(valuation(&ledger, "1000"))
        .unwrap()
        .id;
    meet(&mut devices);
    pause();
    devices[0].delete_valuation(&id).unwrap();
    meet(&mut devices);
    assert!(devices[1].doc.valuations.is_empty());
    pause();
    devices[0]
        .add_valuation(valuation(&ledger, "1200"))
        .unwrap();
    meet(&mut devices);
    for device in &devices {
        assert_eq!(device.doc.valuations.len(), 1);
        assert_eq!(device.doc.valuations[0].unit_price, amount("1200"));
    }
}

#[test]
fn a_valuation_deleted_on_one_device_and_replaced_on_the_other_end_equal() {
    let (_dir, mut devices, ledger) = pair();
    let id = devices[0]
        .add_valuation(valuation(&ledger, "1000"))
        .unwrap()
        .id;
    meet(&mut devices);
    pause();
    devices[0].delete_valuation(&id).unwrap();
    pause();
    devices[1]
        .add_valuation(valuation(&ledger, "1300"))
        .unwrap();
    meet(&mut devices);
    assert_eq!(devices[0].doc.valuations, devices[1].doc.valuations);
}

fn goal_setup() -> (
    tempfile::TempDir,
    UnlockedVault,
    UnlockedVault,
    String,
    String,
) {
    let (dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "Cash", StoreKind::Cash, None);
    left.add_transaction(income(&wallet, "1000", "pay"))
        .unwrap();
    let goal = left
        .create_goal(GoalDraft {
            name: "trip".into(),
            active: true,
            target_amount: amount("1000"),
            currency: "usd".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: Some("2026-12-31T00:00:00Z".into()),
            priority: 3,
            monthly_commitment: None,
            scope: StoreScope {
                store_ids: vec![wallet.clone()],
                include_children: true,
            },
            scenario_id: None,
            expected_return: None,
        })
        .unwrap()
        .id;
    settle(&mut left, &mut right);
    (dir, left, right, wallet, goal)
}

#[test]
fn a_goal_envelope_made_again_after_it_was_deleted_reaches_the_other_device() {
    let (_dir, mut left, mut right, wallet, goal) = goal_setup();
    left.fund_goal(&goal, &wallet, amount("100")).unwrap();
    settle(&mut left, &mut right);
    let id = left.doc.envelopes[0].id.clone();
    pause();
    left.delete_envelope(&id).unwrap();
    settle(&mut left, &mut right);
    assert!(right.doc.envelopes.is_empty());
    pause();
    left.fund_goal(&goal, &wallet, amount("40")).unwrap();
    assert_eq!(
        left.doc.envelopes[0].id, id,
        "the same goal names the same record"
    );
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert_eq!(device.doc.envelopes.len(), 1);
        assert_eq!(device.doc.envelopes[0].allocated, amount("40"));
    }
}

#[test]
fn a_goal_envelope_deleted_on_one_device_and_topped_up_on_the_other_end_equal() {
    let (_dir, mut left, mut right, wallet, goal) = goal_setup();
    left.fund_goal(&goal, &wallet, amount("100")).unwrap();
    settle(&mut left, &mut right);
    let id = left.doc.envelopes[0].id.clone();
    pause();
    left.delete_envelope(&id).unwrap();
    pause();
    right.fund_envelope(&id, amount("30")).unwrap();
    settle(&mut left, &mut right);
    assert_eq!(left.doc.envelopes, right.doc.envelopes);
}
