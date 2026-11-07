//! Lists and maps inside one record merge entry by entry: purchases and
//! people added to the same shared event on two devices, exchange keys,
//! per-currency inflation choices and the accounts of the emergency fund.

use super::tests::{settle, two_devices};
use super::*;
use crate::ipc_contract::{
    SettleSharedInput, SharedEventDraft, SharedItemDraft, SharedParticipantDraft,
    SharedPurchaseDraft,
};
use crate::model::inflation::InflationMode;
use crate::model::shared_event::{PriceMode, SettleDirection};
use crate::model::store::{Binding, StoreKind, SubType};
use crate::service::StoreExtras;
use crate::service::test_support::{amount, income_of, store, usd};

fn guest(name: &str) -> SharedParticipantDraft {
    SharedParticipantDraft {
        name: name.into(),
        ..Default::default()
    }
}

fn thing(title: &str) -> SharedItemDraft {
    SharedItemDraft {
        title: title.into(),
        ..Default::default()
    }
}

/// An event both devices hold, with one guest.
fn shared_event(left: &mut UnlockedVault, right: &mut UnlockedVault) -> String {
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "Picnic".into(),
            currency: "usd".into(),
            participants: vec![guest("Ann")],
            ..Default::default()
        })
        .unwrap();
    settle(left, right);
    event.event.id
}

fn titles(vault: &UnlockedVault, event: &str) -> Vec<String> {
    let mut titles: Vec<String> = vault
        .doc
        .shared_events
        .iter()
        .find(|candidate| candidate.id == event)
        .unwrap()
        .items
        .iter()
        .map(|item| item.title.clone())
        .collect();
    titles.sort();
    titles
}

fn names(vault: &UnlockedVault, event: &str) -> Vec<String> {
    let mut names: Vec<String> = vault
        .doc
        .shared_events
        .iter()
        .find(|candidate| candidate.id == event)
        .unwrap()
        .participants
        .iter()
        .map(|person| person.name.clone())
        .collect();
    names.sort();
    names
}

#[test]
fn concurrent_purchases_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let event = shared_event(&mut left, &mut right);
    left.add_shared_item(&event, thing("beer")).unwrap();
    right.add_shared_item(&event, thing("coal")).unwrap();
    settle(&mut left, &mut right);
    assert_eq!(titles(&left, &event), vec!["beer", "coal"]);
    assert_eq!(titles(&right, &event), vec!["beer", "coal"]);
}

#[test]
fn concurrent_participants_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let event = shared_event(&mut left, &mut right);
    left.add_shared_participant(&event, guest("Bob")).unwrap();
    right.add_shared_participant(&event, guest("Cid")).unwrap();
    settle(&mut left, &mut right);
    assert_eq!(names(&left, &event), vec!["Ann", "Bob", "Cid"]);
    assert_eq!(names(&right, &event), vec!["Ann", "Bob", "Cid"]);
}

#[test]
fn a_removed_participant_and_a_removed_item_propagate() {
    let (_dir, mut left, mut right) = two_devices();
    let event = shared_event(&mut left, &mut right);
    let bob = left
        .add_shared_participant(&event, guest("Bob"))
        .unwrap()
        .event
        .participants
        .iter()
        .find(|person| person.name == "Bob")
        .unwrap()
        .id
        .clone();
    let view = left.add_shared_item(&event, thing("beer")).unwrap();
    let beer = view.event.items[0].id.clone();
    settle(&mut left, &mut right);
    assert_eq!(names(&right, &event), vec!["Ann", "Bob"]);
    // Meanwhile the other device adds someone: only the removal and the
    // addition meet, nobody else is touched.
    right.add_shared_participant(&event, guest("Cid")).unwrap();
    left.remove_shared_participant(&event, &bob).unwrap();
    left.delete_shared_item(&event, &beer).unwrap();
    settle(&mut left, &mut right);
    assert_eq!(names(&left, &event), vec!["Ann", "Cid"]);
    assert_eq!(names(&right, &event), vec!["Ann", "Cid"]);
    assert!(titles(&right, &event).is_empty());
}

#[test]
fn concurrent_settlements_are_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "Bank", StoreKind::Bank, None);
    left.add_transaction(income_of(&wallet, usd(), "1000", "1", "salary"))
        .unwrap();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "Trip".into(),
            currency: "usd".into(),
            participants: vec![
                SharedParticipantDraft {
                    id: Some("ann".into()),
                    name: "Ann".into(),
                    ..Default::default()
                },
                SharedParticipantDraft {
                    id: Some("bob".into()),
                    name: "Bob".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap()
        .event
        .id;
    left.add_shared_purchase(
        &event,
        SharedPurchaseDraft {
            new_item_title: Some("tent".into()),
            qty: amount("1"),
            amount: amount("90"),
            currency: "usd".into(),
            price_mode: PriceMode::Total,
            store_id: Some(wallet),
            ..Default::default()
        },
    )
    .unwrap();
    settle(&mut left, &mut right);
    for (vault, who) in [(&mut left, "ann"), (&mut right, "bob")] {
        vault
            .settle_shared(SettleSharedInput {
                event_id: event.clone(),
                participant_id: who.into(),
                direction: SettleDirection::ToOrganizer,
                amount: amount("10"),
                store_id: None,
                at: None,
            })
            .unwrap();
    }
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        let found = vault
            .doc
            .shared_events
            .iter()
            .find(|candidate| candidate.id == event)
            .unwrap();
        assert_eq!(found.settlements.len(), 2);
    }
}

fn exchange_store(vault: &mut UnlockedVault, name: &str) -> String {
    vault
        .create_store_with(
            name,
            StoreKind::Exchange,
            "#f0b90b",
            "",
            None,
            StoreExtras {
                provider: Some(crate::model::store::ExchangeProvider::Bybit),
                ..Default::default()
            },
        )
        .unwrap()
        .id
}

#[test]
fn an_exchange_key_stays_on_its_device_but_imports_travel() {
    let (_dir, mut left, mut right) = two_devices();
    left.raise_to_level3_for_tests();
    right.raise_to_level3_for_tests();
    let first = exchange_store(&mut left, "Bybit A");
    settle(&mut left, &mut right);
    left.set_exchange_key(&first, "keyA-1234567", "secretA-7654321")
        .unwrap();
    left.stage_imports(vec![crate::model::vault::PendingImport {
        exec_id: "exec-1".into(),
        store_id: first.clone(),
        ..Default::default()
    }])
    .unwrap();
    let manifest = left.sync_manifest(None).unwrap();
    let keys: Vec<String> = manifest.keys().cloned().collect();
    let records = serde_json::to_string(&left.sync_records(&keys).unwrap()).unwrap();
    assert!(!records.contains("secretA-7654321"));
    assert!(!records.contains("keyA-1234567"));
    settle(&mut left, &mut right);

    // The other device has no key of its own and is told so.
    assert!(right.doc.exchanges.bybit.is_empty());
    assert!(right.device_key(&first).unwrap().is_none());
    assert_eq!(
        right.exchange_key_state(&first),
        crate::ipc_contract::security::ExchangeKeyState::Missing
    );
    assert!(!right.has_device_keys());
    let doc = serde_json::to_string(&right.doc).unwrap();
    assert!(!doc.contains("secretA-7654321"));
    // What the key imported still reaches it.
    assert!(
        right
            .pending_imports()
            .iter()
            .any(|import| import.exec_id == "exec-1")
    );
    assert_eq!(
        left.exchange_key_state(&first),
        crate::ipc_contract::security::ExchangeKeyState::NotVerified
    );
}

#[test]
fn inflation_modes_of_two_currencies_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    left.set_inflation_mode("usd", Some(InflationMode::Manual))
        .unwrap();
    right
        .set_inflation_mode("eur", Some(InflationMode::Manual))
        .unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(vault.doc.settings.inflation_mode.len(), 2);
        assert_eq!(
            vault.doc.settings.inflation_mode.get("usd"),
            Some(&InflationMode::Manual)
        );
        assert_eq!(
            vault.doc.settings.inflation_mode.get("eur"),
            Some(&InflationMode::Manual)
        );
    }
}

fn savings(vault: &mut UnlockedVault, name: &str) -> String {
    let bank = store(vault, &format!("{name} bank"), StoreKind::Bank, None);
    let extras = StoreExtras {
        sub_type: Some(SubType::Savings),
        binding: Some(Binding::Asset(usd())),
        ..StoreExtras::default()
    };
    vault
        .create_store_with(name, StoreKind::Bank, "#000", "", Some(bank), extras)
        .unwrap()
        .id
}

fn fund_ids(vault: &UnlockedVault) -> Vec<String> {
    let mut ids = vault.doc.emergency_fund.scope.store_ids.clone();
    ids.sort();
    ids
}

#[test]
fn emergency_accounts_added_on_two_devices_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let first = savings(&mut left, "Reserve A");
    let second = savings(&mut left, "Reserve B");
    settle(&mut left, &mut right);
    left.emergency_fund_toggle(&first, true).unwrap();
    right.emergency_fund_toggle(&second, true).unwrap();
    settle(&mut left, &mut right);
    let mut want = vec![first, second];
    want.sort();
    assert_eq!(fund_ids(&left), want);
    assert_eq!(fund_ids(&right), want);
}

#[test]
fn deleted_store_is_pruned_from_emergency_scope() {
    let (_dir, mut left, mut right) = two_devices();
    let doomed = savings(&mut left, "Doomed");
    let kept = savings(&mut left, "Kept");
    settle(&mut left, &mut right);
    // The right device puts the account in the fund while the left one
    // deletes it: after the merge the fund must not name a missing store.
    right.emergency_fund_toggle(&doomed, true).unwrap();
    right.emergency_fund_toggle(&kept, true).unwrap();
    left.delete_store(&doomed).unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(fund_ids(vault), vec![kept.clone()]);
        assert!(vault.find_store(&doomed).is_none());
    }
    // And the fund can still be edited.
    left.emergency_fund_toggle(&kept, false).unwrap();
    settle(&mut left, &mut right);
    assert!(fund_ids(&right).is_empty());
}

#[test]
fn a_deleted_data_source_is_pruned_from_the_inflation_maps() {
    let (_dir, mut left, mut right) = two_devices();
    let draft = crate::service::datasources::DataSourceDraft {
        id: None,
        name: "CPI".into(),
        url: "https://example.org/cpi".into(),
        method: crate::model::datasource::SourceMethod::Json { path: "v".into() },
        output: crate::model::datasource::SourceOutput::Number,
        number: Default::default(),
        interval: crate::model::datasource::SourceInterval::Day,
        uses: Vec::new(),
    };
    let source = left.save_data_source(draft).unwrap().source.id;
    settle(&mut left, &mut right);
    // The right device points a currency at the source while the left one
    // deletes it.
    right.set_inflation_source("usd", &source).unwrap();
    left.delete_data_source(&source).unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert!(vault.doc.settings.inflation_source.is_empty());
        assert!(vault.doc.settings.inflation_mode.is_empty());
    }
}

#[test]
fn the_last_participant_removed_while_another_is_added() {
    let (_dir, mut left, mut right) = two_devices();
    let event = shared_event(&mut left, &mut right);
    let ann = right.doc.shared_events[0].participants[0].id.clone();
    right.remove_shared_participant(&event, &ann).unwrap();
    left.add_shared_participant(&event, guest("Bob")).unwrap();
    settle(&mut left, &mut right);
    assert_eq!(names(&left, &event), vec!["Bob"]);
    assert_eq!(names(&right, &event), vec!["Bob"]);
}
