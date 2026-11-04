//! Stage P, 100: the flat inflation rates belong to the group. A rate set on
//! one device reaches the others, two devices that copied different rates of
//! their app config settle on one, and a form that did not touch a currency
//! does not overwrite another device's value.

use std::collections::BTreeMap;

use super::tests::{settle, two_devices};
use super::*;
use crate::Amount;
use crate::ipc_contract::UpdateSettingsInput;
use crate::service::test_support::amount;

fn table(pairs: &[(&str, &str)]) -> BTreeMap<String, Amount> {
    pairs
        .iter()
        .map(|(code, rate)| ((*code).to_string(), amount(rate)))
        .collect()
}

fn set_rates(vault: &mut UnlockedVault, pairs: &[(&str, &str)]) {
    vault
        .update_settings(UpdateSettingsInput {
            inflation_manual: Some(table(pairs)),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn a_rate_set_on_one_device_reaches_the_other() {
    let (_dir, mut left, mut right) = two_devices();
    set_rates(&mut left, &[("rub", "9.5"), ("gbp", "4")]);
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let flat = device.flat_inflation();
        assert_eq!(flat.get("rub"), Some(&amount("9.5")));
        assert_eq!(flat.get("gbp"), Some(&amount("4")));
        assert_eq!(flat.get("usd"), Some(&amount("3")));
    }
}

#[test]
fn rates_of_different_currencies_set_in_parallel_both_stay() {
    let (_dir, mut left, mut right) = two_devices();
    set_rates(&mut left, &[("rub", "9.5")]);
    set_rates(&mut right, &[("gbp", "4")]);
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let flat = device.flat_inflation();
        assert_eq!(flat.get("rub"), Some(&amount("9.5")));
        assert_eq!(flat.get("gbp"), Some(&amount("4")));
    }
}

#[test]
fn a_form_that_kept_a_currency_does_not_overwrite_another_devices_value() {
    let (_dir, mut left, mut right) = two_devices();
    set_rates(&mut left, &[("rub", "9.5")]);
    settle(&mut left, &mut right);
    // The right device changes THB; its form sends the whole table, rub as it was.
    set_rates(&mut right, &[("rub", "9.5"), ("thb", "2")]);
    // Meanwhile the left device changes rub.
    set_rates(&mut left, &[("rub", "11")]);
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let flat = device.flat_inflation();
        assert_eq!(flat.get("rub"), Some(&amount("11")));
        assert_eq!(flat.get("thb"), Some(&amount("2")));
    }
}

/// "Adopted" says this device gave up its own table: a peer's flag must not
/// throw away the rates of a device that has not done so yet.
#[test]
fn a_peer_that_adopted_first_does_not_cancel_this_devices_adoption() {
    let (_dir, mut left, mut right) = two_devices();
    left.adopt_flat_inflation(&table(&[("rub", "10")])).unwrap();
    settle(&mut left, &mut right);
    assert!(
        right
            .adopt_flat_inflation(&table(&[("rub", "12"), ("gbp", "5")]))
            .unwrap()
    );
    settle(&mut left, &mut right);
    let flat = left.flat_inflation();
    assert_eq!(flat, right.flat_inflation());
    assert_eq!(flat.get("gbp"), Some(&amount("5")));
    // The group's value of a currency stays.
    assert_eq!(flat.get("rub"), Some(&amount("10")));
}

#[test]
fn two_devices_that_adopted_different_values_settle_on_one() {
    let (_dir, mut left, mut right) = two_devices();
    assert!(
        left.adopt_flat_inflation(&table(&[("rub", "10"), ("usd", "3")]))
            .unwrap()
    );
    assert!(
        right
            .adopt_flat_inflation(&table(&[("rub", "12"), ("gbp", "5")]))
            .unwrap()
    );
    settle(&mut left, &mut right);
    assert_eq!(left.flat_inflation(), right.flat_inflation());
    let flat = left.flat_inflation();
    assert!(
        flat.get("rub") == Some(&amount("10")) || flat.get("rub") == Some(&amount("12")),
        "{flat:?}"
    );
    // The currency only one of them had is kept.
    assert_eq!(flat.get("gbp"), Some(&amount("5")));
    // Settled: a further round moves nothing.
    assert_eq!(super::tests::exchange(&mut left, &mut right), (None, None));
}
