//! The terminal of an exchange belongs to the device: it never travels, never
//! counts as a change and is never overwritten. The import settings beside it
//! still sync. A car's mileage and its insurance are separate fields.

use chrono::NaiveDate;

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::model::store::{
    CarInfo, ExchangeProvider, PropertyCategory, PropertyInfo, RevalueRule, StoreKind,
    TerminalState,
};
use crate::service::test_support::amount;
use crate::service::{StoreExtras, TerminalPatch};
use crate::sync::registry::find;
use crate::sync::stamp::Clock;
use crate::sync::state::SyncState;
use crate::sync::track::Tracker;

fn exchange_store(vault: &mut UnlockedVault) -> String {
    vault
        .create_store_with(
            "Bybit",
            StoreKind::Exchange,
            "",
            "",
            None,
            StoreExtras {
                provider: Some(ExchangeProvider::Bybit),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id
}

fn terminal(vault: &UnlockedVault, id: &str) -> TerminalState {
    vault.terminal_state(id).unwrap()
}

fn set_timeframe(vault: &mut UnlockedVault, id: &str, timeframe: &str) {
    vault
        .update_terminal_state(
            id,
            TerminalPatch {
                timeframe: Some(timeframe.into()),
                ..TerminalPatch::default()
            },
        )
        .unwrap();
}

fn set_watchlist(vault: &mut UnlockedVault, id: &str, symbol: &str) {
    vault
        .update_terminal_state(
            id,
            TerminalPatch {
                watchlist: Some(vec![symbol.into()]),
                ..TerminalPatch::default()
            },
        )
        .unwrap();
}

fn coins_of(vault: &UnlockedVault, id: &str) -> Vec<String> {
    vault.import_settings(id).unwrap().coins
}

fn set_coins(vault: &mut UnlockedVault, id: &str, coins: &[&str]) {
    let mut settings = vault.import_settings(id).unwrap();
    settings.coins = coins.iter().map(|coin| (*coin).to_string()).collect();
    vault.set_import_settings(id, settings).unwrap();
}

#[test]
fn terminal_state_is_not_synced() {
    let (_dir, mut left, mut right) = two_devices();
    let id = exchange_store(&mut left);
    settle(&mut left, &mut right);

    set_watchlist(&mut left, &id, "SOLUSDT");
    left.save_terminal_drawings(&id, "SOLUSDT", r#"[{"id":"1"}]"#)
        .unwrap();
    set_timeframe(&mut right, &id, "4h");
    set_coins(&mut left, &id, &["BTC"]);
    settle(&mut left, &mut right);

    let on_left = terminal(&left, &id);
    assert_eq!(on_left.watchlist, vec!["SOLUSDT".to_string()]);
    assert!(on_left.drawings.contains_key("SOLUSDT"));
    assert_eq!(on_left.timeframe, TerminalState::default().timeframe);
    let on_right = terminal(&right, &id);
    assert_eq!(on_right.timeframe, "4h");
    assert_eq!(on_right.watchlist, TerminalState::default().watchlist);
    assert!(on_right.drawings.is_empty());
    // The import settings beside it did travel.
    assert_eq!(coins_of(&right, &id), vec!["BTC".to_string()]);
}

#[test]
fn terminal_change_stamps_nothing() {
    let (_dir, mut vault, _right) = two_devices();
    let id = exchange_store(&mut vault);
    let mut tracker = Tracker::capture(&vault.doc_value().unwrap());
    let mut state = SyncState::default();
    let mut clock = Clock::new("dev");

    set_watchlist(&mut vault, &id, "SOLUSDT");
    set_timeframe(&mut vault, &id, "15m");
    vault
        .save_terminal_drawings(&id, "SOLUSDT", r#"[{"id":"1"}]"#)
        .unwrap();
    let changed = tracker.stamp(&vault.doc_value().unwrap(), &mut state, &mut clock);
    assert!(changed.is_empty(), "{changed:?}");
    assert!(state.stamps.is_empty(), "{:?}", state.stamps);

    // An import setting does count.
    set_coins(&mut vault, &id, &["ETH"]);
    let changed = tracker.stamp(&vault.doc_value().unwrap(), &mut state, &mut clock);
    assert_eq!(changed, vec![format!("stores:{id}")]);
    let stamps = &state.stamps[&format!("stores:{id}")];
    assert!(stamps.fields.contains_key("exchange.import"));
    assert!(!stamps.fields.keys().any(|name| name.contains("terminal")));
}

#[test]
fn digests_agree_with_different_terminals() {
    let (_dir, mut left, mut right) = two_devices();
    let id = exchange_store(&mut left);
    settle(&mut left, &mut right);
    set_watchlist(&mut left, &id, "SOLUSDT");
    set_timeframe(&mut right, &id, "1d");
    assert_ne!(terminal(&left, &id), terminal(&right, &id));
    assert_eq!(
        left.sync_manifest(None).unwrap(),
        right.sync_manifest(None).unwrap()
    );
    // Nothing is left to send either way.
    assert_eq!(exchange(&mut left, &mut right), (None, None));
}

#[test]
fn undo_of_a_batch_keeps_the_local_terminal() {
    let (_dir, mut left, mut right) = two_devices();
    let id = exchange_store(&mut left);
    settle(&mut left, &mut right);

    set_coins(&mut left, &id, &["BTC"]);
    set_timeframe(&mut right, &id, "4h");
    let (_, applied) = exchange(&mut left, &mut right);
    let batch = applied.expect("the right device changed");
    assert_eq!(coins_of(&right, &id), vec!["BTC".to_string()]);
    set_watchlist(&mut right, &id, "DOGEUSDT");

    let report = right.sync_undo(&batch.id).unwrap();
    assert_eq!(report.restored, 1, "{report:?}");
    assert!(coins_of(&right, &id).is_empty());
    let kept = terminal(&right, &id);
    assert_eq!(kept.timeframe, "4h");
    assert_eq!(kept.watchlist, vec!["DOGEUSDT".to_string()]);
}

#[test]
fn a_new_store_from_a_peer_gets_a_default_terminal() {
    let (_dir, mut left, mut right) = two_devices();
    let id = exchange_store(&mut left);
    set_timeframe(&mut left, &id, "1w");
    set_watchlist(&mut left, &id, "SOLUSDT");
    settle(&mut left, &mut right);
    assert_eq!(terminal(&right, &id), TerminalState::default());
    assert_eq!(terminal(&left, &id).timeframe, "1w");
    // A record from an older device, which still sends its terminal inside.
    let mut value = left.doc_value().unwrap();
    let stores = value["stores"].as_array_mut().unwrap();
    let sent = stores.iter_mut().find(|store| store["id"] == id).unwrap();
    assert_eq!(sent["exchange"]["terminal"]["timeframe"], "1w");
    let incoming = sent.clone();
    let mut doc = value.clone();
    view::write(&mut doc, &format!("stores:{id}"), None);
    view::write(&mut doc, &format!("stores:{id}"), Some(&incoming));
    let stored = view::read(&doc, &format!("stores:{id}")).unwrap();
    assert!(stored["exchange"].get("terminal").is_none());
    let rebuilt: crate::VaultDoc = serde_json::from_value(doc).unwrap();
    let store = rebuilt.stores.iter().find(|store| store.id == id).unwrap();
    assert_eq!(
        store.exchange.as_ref().unwrap().terminal,
        TerminalState::default()
    );
}

fn car_store(vault: &mut UnlockedVault) -> String {
    vault
        .create_store_with(
            "Octavia",
            StoreKind::Property,
            "#555",
            "",
            None,
            StoreExtras {
                property: Some(PropertyInfo {
                    category: PropertyCategory::Car,
                    car: Some(CarInfo::default()),
                    ..PropertyInfo::default()
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id
}

fn car_of(vault: &UnlockedVault, id: &str) -> CarInfo {
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
fn mileage_and_insurance_edited_on_two_devices_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let id = car_store(&mut left);
    settle(&mut left, &mut right);

    let day = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
    left.vehicle_update_mileage(&id, 42_000, day).unwrap();
    for store in &mut right.doc.stores {
        if store.id == id {
            let car = store.property.as_mut().unwrap().car.as_mut().unwrap();
            car.osago_until = Some("2027-01-01".into());
        }
    }
    right.persist().unwrap();
    settle(&mut left, &mut right);

    for vault in [&left, &right] {
        let car = car_of(vault, &id);
        assert_eq!(car.mileage_km, Some(42_000));
        assert_eq!(car.osago_until.as_deref(), Some("2027-01-01"));
    }
}

#[test]
fn revalue_and_car_edited_on_two_devices_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let id = car_store(&mut left);
    settle(&mut left, &mut right);

    let day = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
    left.vehicle_update_mileage(&id, 7_000, day).unwrap();
    right
        .set_revalue_rule(
            &id,
            Some(RevalueRule {
                pct_per_year: Some(amount("5")),
                floor: None,
            }),
            "UTC",
        )
        .unwrap();
    settle(&mut left, &mut right);

    for vault in [&left, &right] {
        let info = vault.find_store(&id).unwrap().property.clone().unwrap();
        assert_eq!(info.car.unwrap().mileage_km, Some(7_000));
        assert_eq!(info.revalue.unwrap().pct_per_year, Some(amount("5")));
    }
}

#[test]
fn the_terminal_is_a_device_field_of_stores() {
    let coll = find("stores").unwrap();
    assert!(coll.skip.contains(&"exchange.terminal"));
}
