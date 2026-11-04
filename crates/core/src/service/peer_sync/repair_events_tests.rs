//! Catalog 123 and 116: guests that became contacts, payments linked to events,
//! the currency and the organizer of an event after a merge.

use super::tests::{exchange, settle, two_devices};
use super::*;
use crate::ipc_contract::SettleSharedInput;
use crate::ipc_contract::{
    AttachPaymentInput, SharedEventDraft, SharedParticipantDraft, SharedPurchaseDraft,
};
use crate::model::contact::{Contact, ContactKind};
use crate::model::shared_event::{PriceMode, Purchase, Settlement, SettlementKind};
use crate::model::store::StoreKind;
use crate::service::sync_ids;
use crate::service::test_support::{amount, empty_vault, payment, store};

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(3));
}

fn event_with_guest(vault: &mut UnlockedVault, title: &str, currency: &str) -> (String, String) {
    let event = vault
        .create_shared_event(SharedEventDraft {
            title: title.into(),
            currency: currency.into(),
            participants: vec![SharedParticipantDraft {
                name: "ann".into(),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap()
        .event;
    (event.id, event.participants[0].id.clone())
}

fn contacts_named(vault: &UnlockedVault, name: &str) -> Vec<Contact> {
    vault
        .doc
        .contacts
        .iter()
        .filter(|contact| contact.name == name)
        .cloned()
        .collect()
}

fn paid(vault: &mut UnlockedVault) -> String {
    let wallet = store(vault, "bank", StoreKind::Bank, None);
    let tx = payment(&wallet, "10", "taxi");
    let id = tx.meta().id.clone();
    vault.add_transaction(tx).unwrap();
    id
}

fn attach(vault: &mut UnlockedVault, event_id: &str, tx_id: &str, title: &str) {
    vault
        .attach_payment_to_shared_event(AttachPaymentInput {
            event_id: event_id.into(),
            tx_id: tx_id.into(),
            new_item_title: Some(title.into()),
            ..Default::default()
        })
        .unwrap();
}

/// `(event id, purchase id)` of every link purchase of the operation.
fn links_of(vault: &UnlockedVault, tx_id: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for event in &vault.doc.shared_events {
        for purchase in event.items.iter().flat_map(|item| &item.purchases) {
            if purchase.tx_id.as_deref() == Some(tx_id) {
                out.push((event.id.clone(), purchase.id.clone()));
            }
        }
    }
    out
}

#[test]
fn a_guest_made_a_contact_on_two_devices_is_one_contact_on_both() {
    let (_dir, mut left, mut right) = two_devices();
    let (event_id, guest) = event_with_guest(&mut left, "trip", "usd");
    settle(&mut left, &mut right);
    left.make_shared_participant_contact(&event_id, &guest)
        .unwrap();
    pause();
    right
        .make_shared_participant_contact(&event_id, &guest)
        .unwrap();
    settle(&mut left, &mut right);
    let wanted = format!("guest-{}", sync_ids::sync_safe_id(&[&event_id, &guest]));
    for device in [&left, &right] {
        let found = contacts_named(device, "ann");
        assert_eq!(found.len(), 1, "one contact for one guest");
        assert_eq!(found[0].id, wanted);
        assert_eq!(
            device.doc.shared_events[0].participants[0]
                .contact_id
                .as_deref(),
            Some(wanted.as_str())
        );
    }
}

#[test]
fn an_old_random_guest_contact_folds_into_the_deterministic_one() {
    let (_dir, mut left, mut right) = two_devices();
    let (event_id, guest) = event_with_guest(&mut left, "trip", "usd");
    settle(&mut left, &mut right);
    let wanted = left
        .make_shared_participant_contact(&event_id, &guest)
        .unwrap()
        .id;
    let wallet = store(&mut right, "bank", StoreKind::Bank, None);
    pause();
    // The older build: a random id, linked later than the new contact.
    right.doc.contacts.push(Contact {
        id: "00000000-old".into(),
        name: "ann".into(),
        kind: ContactKind::Person,
        ..Contact::default()
    });
    right.doc.shared_events[0].participants[0].contact_id = Some("00000000-old".into());
    let mut tx = payment(&wallet, "5", "to ann");
    tx.meta_mut().contact_id = Some("00000000-old".into());
    let tx_id = tx.meta().id.clone();
    right.add_transaction(tx).unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let found = contacts_named(device, "ann");
        assert_eq!(found.len(), 1, "the old contact is folded away");
        assert_eq!(found[0].id, wanted);
        assert_eq!(
            device.doc.shared_events[0].participants[0]
                .contact_id
                .as_deref(),
            Some(wanted.as_str())
        );
        let moved = device
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == tx_id)
            .unwrap();
        assert_eq!(moved.meta().contact_id.as_deref(), Some(wanted.as_str()));
    }
}

#[test]
fn one_operation_attached_twice_to_one_event_is_one_link() {
    let (_dir, mut left, mut right) = two_devices();
    let (event_id, _) = event_with_guest(&mut left, "trip", "usd");
    let tx_id = paid(&mut left);
    settle(&mut left, &mut right);
    attach(&mut left, &event_id, &tx_id, "taxi left");
    pause();
    attach(&mut right, &event_id, &tx_id, "taxi right");
    settle(&mut left, &mut right);
    let wanted = format!("link-{}", sync_ids::sync_safe_id(&[&tx_id]));
    for device in [&left, &right] {
        let links = links_of(device, &tx_id);
        assert_eq!(links.len(), 1, "one link: {links:?}");
        assert_eq!(links[0].1, wanted);
    }
    assert_eq!(links_of(&left, &tx_id), links_of(&right, &tx_id));
}

#[test]
fn one_operation_linked_to_two_events_keeps_the_earliest_link_on_both() {
    let (_dir, mut left, mut right) = two_devices();
    let (first, _) = event_with_guest(&mut left, "first", "usd");
    let (second, _) = event_with_guest(&mut left, "second", "usd");
    let tx_id = paid(&mut left);
    settle(&mut left, &mut right);
    attach(&mut left, &first, &tx_id, "taxi");
    pause();
    attach(&mut right, &second, &tx_id, "taxi");
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let links = links_of(device, &tx_id);
        assert_eq!(links.len(), 1, "one link: {links:?}");
        assert_eq!(links[0].0, first, "the earlier link stands");
        let body = device
            .doc
            .notification_log
            .iter()
            .map(|record| record.body.clone())
            .collect::<String>();
        assert!(
            body.contains("операция была привязана к двум событиям"),
            "{body}"
        );
    }
}

#[test]
fn a_currency_changed_after_a_purchase_goes_back_on_both_devices() {
    let (_dir, mut left, mut right) = two_devices();
    let (event_id, guest) = event_with_guest(&mut left, "trip", "usd");
    settle(&mut left, &mut right);
    let event = left.doc.shared_events[0].clone();
    left.update_shared_event(
        &event_id,
        SharedEventDraft {
            title: event.title.clone(),
            currency: "eur".into(),
            rounding_step: Some(event.rounding_step),
            organizer: event.organizer.clone(),
            i_participate: true,
            ..Default::default()
        },
    )
    .unwrap();
    pause();
    right
        .add_shared_purchase(
            &event_id,
            SharedPurchaseDraft {
                new_item_title: Some("taxi".into()),
                buyer: guest,
                qty: amount("1"),
                amount: amount("10"),
                currency: "usd".into(),
                price_mode: PriceMode::Total,
                ..Default::default()
            },
        )
        .unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert_eq!(device.doc.shared_events[0].currency, "usd");
    }
    assert_eq!(exchange(&mut left, &mut right), (None, None));
}

fn purchase_in(currency: &str, id: &str) -> Purchase {
    Purchase {
        id: id.into(),
        buyer: "me".into(),
        qty: amount("1"),
        amount: amount("10"),
        currency: currency.into(),
        price_mode: PriceMode::Total,
        rate_to_event: amount("1"),
        ..Purchase::default()
    }
}

#[test]
fn ambiguous_currencies_change_nothing_and_name_the_purchases() {
    let (_dir, mut vault) = empty_vault();
    let (event_id, _) = event_with_guest(&mut vault, "trip", "usd");
    let event = vault
        .doc
        .shared_events
        .iter_mut()
        .find(|event| event.id == event_id)
        .unwrap();
    event.items.push(crate::model::shared_event::EventItem {
        id: "item".into(),
        title: "things".into(),
        purchases: vec![purchase_in("eur", "p-eur"), purchase_in("gbp", "p-gbp")],
        ..Default::default()
    });
    vault.persist().unwrap();
    let report = vault.repair_invariants(None).unwrap();
    assert_eq!(vault.doc.shared_events[0].currency, "usd");
    let mut named = report.needs_rate.clone();
    named.sort();
    assert_eq!(named, vec!["p-eur".to_string(), "p-gbp".to_string()]);
}

#[test]
fn an_organizer_changed_after_a_settlement_goes_back_on_both_devices() {
    let (_dir, mut left, mut right) = two_devices();
    let (event_id, guest) = event_with_guest(&mut left, "trip", "usd");
    let wallet = store(&mut left, "bank", StoreKind::Bank, None);
    settle(&mut left, &mut right);
    let event = left.doc.shared_events[0].clone();
    left.update_shared_event(
        &event_id,
        SharedEventDraft {
            title: event.title.clone(),
            currency: "usd".into(),
            rounding_step: Some(event.rounding_step),
            organizer: guest.clone(),
            i_participate: true,
            ..Default::default()
        },
    )
    .unwrap();
    pause();
    right
        .add_shared_purchase(
            &event_id,
            SharedPurchaseDraft {
                new_item_title: Some("taxi".into()),
                buyer: "me".into(),
                qty: amount("1"),
                amount: amount("10"),
                price_mode: PriceMode::Total,
                store_id: Some(wallet),
                ..Default::default()
            },
        )
        .unwrap();
    right
        .settle_shared(SettleSharedInput {
            event_id: event_id.clone(),
            participant_id: guest.clone(),
            direction: crate::model::shared_event::SettleDirection::ToOrganizer,
            amount: amount("2"),
            store_id: None,
            at: None,
        })
        .unwrap();
    assert_eq!(
        right.doc.shared_events[0].settlements[0]
            .organizer
            .as_deref(),
        Some("me")
    );
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert_eq!(device.doc.shared_events[0].organizer, "me");
    }
}

#[test]
fn a_settlement_without_the_organizer_field_is_only_flagged() {
    let (_dir, mut vault) = empty_vault();
    let (event_id, guest) = event_with_guest(&mut vault, "trip", "usd");
    let event = vault
        .doc
        .shared_events
        .iter_mut()
        .find(|event| event.id == event_id)
        .unwrap();
    event.organizer = guest.clone();
    event.settlements.push(Settlement {
        id: "s1".into(),
        kind: SettlementKind::Offset,
        participant_id: guest,
        amount: amount("1"),
        ..Settlement::default()
    });
    vault.persist().unwrap();
    let before = vault.doc.shared_events[0].clone();
    let report = vault.repair_invariants(None).unwrap();
    assert_eq!(vault.doc.shared_events[0], before, "nothing changed");
    assert_eq!(report.organizer_flags, vec![event_id]);
}

#[test]
fn undoing_the_batch_brings_the_removed_link_back() {
    let (_dir, mut left, mut right) = two_devices();
    let (first, _) = event_with_guest(&mut left, "first", "usd");
    let (second, _) = event_with_guest(&mut left, "second", "usd");
    let tx_id = paid(&mut left);
    settle(&mut left, &mut right);
    attach(&mut left, &first, &tx_id, "taxi");
    pause();
    attach(&mut right, &second, &tx_id, "taxi");
    let (_, applied) = exchange(&mut left, &mut right);
    let batch = applied.expect("right took something").id;
    assert_eq!(links_of(&right, &tx_id).len(), 1, "repaired");
    assert!(right.sync_undo(&batch).unwrap().restored > 0);
    let links = links_of(&right, &tx_id);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].0, second, "the repair went with the batch");
}

#[test]
fn a_second_pass_changes_and_stamps_nothing() {
    let (_dir, mut left, mut right) = two_devices();
    let (first, _) = event_with_guest(&mut left, "first", "usd");
    let (second, _) = event_with_guest(&mut left, "second", "usd");
    let tx_id = paid(&mut left);
    settle(&mut left, &mut right);
    attach(&mut left, &first, &tx_id, "taxi");
    attach(&mut right, &second, &tx_id, "taxi");
    settle(&mut left, &mut right);
    let stamps = left.doc.sync.stamps.clone();
    let log = left.doc.notification_log.len();
    assert!(left.repair_invariants(None).unwrap().is_empty());
    assert_eq!(left.doc.sync.stamps, stamps);
    assert_eq!(left.doc.notification_log.len(), log);
}
