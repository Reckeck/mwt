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
    let event = crate::usecases::household::shared_events::create_shared_event(
        left,
        SharedEventDraft {
            title: "Picnic".into(),
            currency: "usd".into(),
            participants: vec![guest("Ann")],
            ..Default::default()
        },
    )
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
    crate::usecases::household::shared_events::add_shared_item(&mut left, &event, thing("beer"))
        .unwrap();
    crate::usecases::household::shared_events::add_shared_item(&mut right, &event, thing("coal"))
        .unwrap();
    settle(&mut left, &mut right);
    assert_eq!(titles(&left, &event), vec!["beer", "coal"]);
    assert_eq!(titles(&right, &event), vec!["beer", "coal"]);
}

#[test]
fn concurrent_participants_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let event = shared_event(&mut left, &mut right);
    crate::usecases::household::shared_events::add_shared_participant(
        &mut left,
        &event,
        guest("Bob"),
    )
    .unwrap();
    crate::usecases::household::shared_events::add_shared_participant(
        &mut right,
        &event,
        guest("Cid"),
    )
    .unwrap();
    settle(&mut left, &mut right);
    assert_eq!(names(&left, &event), vec!["Ann", "Bob", "Cid"]);
    assert_eq!(names(&right, &event), vec!["Ann", "Bob", "Cid"]);
}

#[test]
fn a_removed_participant_and_a_removed_item_propagate() {
    let (_dir, mut left, mut right) = two_devices();
    let event = shared_event(&mut left, &mut right);
    let bob = crate::usecases::household::shared_events::add_shared_participant(
        &mut left,
        &event,
        guest("Bob"),
    )
    .unwrap()
    .event
    .participants
    .iter()
    .find(|person| person.name == "Bob")
    .unwrap()
    .id
    .clone();
    let view = crate::usecases::household::shared_events::add_shared_item(
        &mut left,
        &event,
        thing("beer"),
    )
    .unwrap();
    let beer = view.event.items[0].id.clone();
    settle(&mut left, &mut right);
    assert_eq!(names(&right, &event), vec!["Ann", "Bob"]);
    // Meanwhile the other device adds someone: only the removal and the
    // addition meet, nobody else is touched.
    crate::usecases::household::shared_events::add_shared_participant(
        &mut right,
        &event,
        guest("Cid"),
    )
    .unwrap();
    crate::usecases::household::shared_events::remove_shared_participant(&mut left, &event, &bob)
        .unwrap();
    crate::usecases::household::shared_events::delete_shared_item(&mut left, &event, &beer)
        .unwrap();
    settle(&mut left, &mut right);
    assert_eq!(names(&left, &event), vec!["Ann", "Cid"]);
    assert_eq!(names(&right, &event), vec!["Ann", "Cid"]);
    assert!(titles(&right, &event).is_empty());
}

#[test]
fn concurrent_settlements_are_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let wallet = store(&mut left, "Bank", StoreKind::Bank, None);
    crate::usecases::money::transactions::add_transaction(
        &mut left,
        income_of(&wallet, usd(), "1000", "1", "salary"),
    )
    .unwrap();
    let event = crate::usecases::household::shared_events::create_shared_event(
        &mut left,
        SharedEventDraft {
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
        },
    )
    .unwrap()
    .event
    .id;
    crate::usecases::household::shared_events::add_shared_purchase(
        &mut left,
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
        crate::usecases::household::shared_events::settle_shared(
            vault,
            SettleSharedInput {
                event_id: event.clone(),
                participant_id: who.into(),
                direction: SettleDirection::ToOrganizer,
                amount: amount("10"),
                store_id: None,
                at: None,
            },
        )
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
    crate::usecases::money::stores::create_store_with(
        vault,
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
    crate::usecases::cryptotrack::exchange_import::set_exchange_key(
        &mut left,
        &first,
        "keyA-1234567",
        "secretA-7654321",
    )
    .unwrap();
    crate::usecases::cryptotrack::exchange_import::stage_imports(
        &mut left,
        vec![crate::model::vault::PendingImport {
            exec_id: "exec-1".into(),
            store_id: first.clone(),
            ..Default::default()
        }],
    )
    .unwrap();
    let manifest = crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
    let keys: Vec<String> = manifest.keys().cloned().collect();
    let records = serde_json::to_string(
        &crate::usecases::sync::read::sync_records(&mut left, &keys).unwrap(),
    )
    .unwrap();
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
        crate::cryptotrack::exchange_import::pending_imports(&right.doc)
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
    crate::usecases::money::inflation::set_inflation_mode(
        &mut left,
        "usd",
        Some(InflationMode::Manual),
    )
    .unwrap();
    crate::usecases::money::inflation::set_inflation_mode(
        &mut right,
        "eur",
        Some(InflationMode::Manual),
    )
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
    crate::usecases::money::stores::create_store_with(
        vault,
        name,
        StoreKind::Bank,
        "#000",
        "",
        Some(bank),
        extras,
    )
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
    crate::usecases::planning::emergency::emergency_fund_toggle(&mut left, &first, true).unwrap();
    crate::usecases::planning::emergency::emergency_fund_toggle(&mut right, &second, true).unwrap();
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
    crate::usecases::planning::emergency::emergency_fund_toggle(&mut right, &doomed, true).unwrap();
    crate::usecases::planning::emergency::emergency_fund_toggle(&mut right, &kept, true).unwrap();
    crate::usecases::money::stores::delete_store(&mut left, &doomed).unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(fund_ids(vault), vec![kept.clone()]);
        assert!(crate::money::tree::find_store(&vault.doc, &doomed).is_none());
    }
    // And the fund can still be edited.
    crate::usecases::planning::emergency::emergency_fund_toggle(&mut left, &kept, false).unwrap();
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
    let source = crate::usecases::planning::datasources::save_data_source(&mut left, draft)
        .unwrap()
        .source
        .id;
    settle(&mut left, &mut right);
    // The right device points a currency at the source while the left one
    // deletes it.
    crate::usecases::money::inflation::set_inflation_source(&mut right, "usd", &source).unwrap();
    crate::usecases::planning::datasources::delete_data_source(&mut left, &source).unwrap();
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
    crate::usecases::household::shared_events::remove_shared_participant(&mut right, &event, &ann)
        .unwrap();
    crate::usecases::household::shared_events::add_shared_participant(
        &mut left,
        &event,
        guest("Bob"),
    )
    .unwrap();
    settle(&mut left, &mut right);
    assert_eq!(names(&left, &event), vec!["Bob"]);
    assert_eq!(names(&right, &event), vec!["Bob"]);
}
