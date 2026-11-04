//! R3 for payments: a payment that a shared purchase or settlement names comes
//! back after a parallel delete; an ordinary delete is not undone.

use super::tests::{settle, two_devices};
use super::*;
use crate::ipc_contract::{
    AttachPaymentInput, SettleSharedInput, SharedEventDraft, SharedParticipantDraft,
    SharedPurchaseDraft,
};
use crate::model::shared_event::{PriceMode, SettleDirection};
use crate::model::transaction::Transaction;
use crate::service::test_support::{amount, income, payment};

fn peer(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

fn guest(name: &str) -> SharedParticipantDraft {
    SharedParticipantDraft {
        name: name.into(),
        ..Default::default()
    }
}

fn balance(vault: &UnlockedVault, store_id: &str) -> crate::Amount {
    vault
        .doc
        .holdings
        .iter()
        .filter(|holding| holding.store_id == store_id)
        .map(|holding| holding.quantity)
        .fold(crate::Amount::ZERO, |sum, quantity| sum + quantity)
}

fn has_tx(vault: &UnlockedVault, tx_id: &str) -> bool {
    vault
        .doc
        .transactions
        .iter()
        .any(|tx| tx.meta().id == tx_id)
}

/// Two synced devices, a wallet with 100 and a 3 payment, an event with a guest.
fn setup() -> (
    tempfile::TempDir,
    UnlockedVault,
    UnlockedVault,
    String,
    String,
    String,
) {
    let (dir, mut left, mut right) = two_devices();
    let store_id = left.doc.stores[0].id.clone();
    left.add_transaction(income(&store_id, "100", "start"))
        .unwrap();
    let paid = left
        .add_transaction(payment(&store_id, "3", "taxi"))
        .unwrap();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![guest("ann")],
            ..Default::default()
        })
        .unwrap()
        .event;
    settle(&mut left, &mut right);
    (dir, left, right, store_id, event.id, paid.meta().id.clone())
}

fn attach(vault: &mut UnlockedVault, event_id: &str, tx_id: &str) {
    vault
        .attach_payment_to_shared_event(AttachPaymentInput {
            event_id: event_id.into(),
            tx_id: tx_id.into(),
            new_item_title: Some("attached".into()),
            ..Default::default()
        })
        .unwrap();
}

fn linked(vault: &UnlockedVault, tx_id: &str) -> bool {
    vault.doc.shared_events[0]
        .items
        .iter()
        .flat_map(|item| &item.purchases)
        .any(|purchase| purchase.tx_id.as_deref() == Some(tx_id))
}

#[test]
fn a_payment_deleted_beside_its_attachment_comes_back_with_its_balance() {
    let (_dir, mut left, mut right, store_id, event_id, tx_id) = setup();
    left.delete_transaction(&tx_id).unwrap();
    attach(&mut right, &event_id, &tx_id);
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert!(has_tx(device, &tx_id));
        assert!(linked(device, &tx_id));
        assert_eq!(balance(device, &store_id), amount("97"));
    }
    let tx_left = left
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == tx_id);
    let tx_right = right
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == tx_id);
    assert_eq!(tx_left, tx_right);
    // A second pass changes nothing.
    for device in [&mut left, &mut right] {
        let doc = device.doc.clone();
        let report = device.repair_invariants(None).unwrap();
        assert!(report.restored.is_empty() && report.unrestored.is_empty());
        assert_eq!(device.doc.transactions, doc.transactions);
        assert_eq!(device.doc.shared_events, doc.shared_events);
    }
}

#[test]
fn an_ordinary_delete_of_a_linked_payment_is_not_undone() {
    let (_dir, mut left, mut right, store_id, event_id, tx_id) = setup();
    attach(&mut left, &event_id, &tx_id);
    settle(&mut left, &mut right);
    assert!(linked(&right, &tx_id));
    left.delete_transaction(&tx_id).unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert!(!has_tx(device, &tx_id));
        assert!(!linked(device, &tx_id));
        assert_eq!(balance(device, &store_id), amount("100"));
    }
}

#[test]
fn a_device_without_the_value_leaves_the_purchase_alone_and_reports_it() {
    let (_dir, mut left, mut right, _store, event_id, tx_id) = setup();
    left.delete_transaction(&tx_id).unwrap();
    attach(&mut right, &event_id, &tx_id);
    // Only the attachment travels to the device that deleted the payment.
    let from = right.sync_manifest(None).unwrap();
    let to = left.sync_manifest(None).unwrap();
    let (wants, _) = left.sync_diff(&to, &from);
    let records = right.sync_records(&wants).unwrap();
    left.sync_apply(&records, &peer("right")).unwrap();
    let report = left.repair_invariants(None).unwrap();
    assert!(report.unrestored.contains(&tx_id), "{report:?}");
    assert!(linked(&left, &tx_id), "the purchase is untouched");
    assert!(!has_tx(&left, &tx_id), "no stub payment is invented");
}

/// A settlement that moved money names its transfer like a purchase names its
/// payment: the transfer is deleted on one device while the other edits it.
#[test]
fn a_settlement_never_names_a_transfer_that_is_gone() {
    let (_dir, mut left, mut right, store_id, event_id, _paid) = setup();
    let person = left.doc.shared_events[0].participants[0].id.clone();
    left.make_shared_participant_contact(&event_id, &person)
        .unwrap();
    left.add_shared_purchase(
        &event_id,
        SharedPurchaseDraft {
            new_item_title: Some("dinner".into()),
            buyer: "me".into(),
            qty: amount("1"),
            amount: amount("10"),
            price_mode: PriceMode::Total,
            store_id: Some(store_id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    left.settle_shared(SettleSharedInput {
        event_id: event_id.clone(),
        participant_id: person,
        direction: SettleDirection::ToOrganizer,
        amount: amount("2"),
        store_id: Some(store_id.clone()),
        at: None,
    })
    .unwrap();
    settle(&mut left, &mut right);
    let tx_id = left.doc.shared_events[0].settlements[0]
        .tx_id
        .clone()
        .expect("the settlement moved money");
    left.delete_transaction(&tx_id).unwrap();
    let mut edited = right
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == tx_id)
        .cloned()
        .unwrap();
    if let Transaction::Transfer(transfer) = &mut edited {
        transfer.amount = amount("1");
    }
    let _ = right.update_transaction(&tx_id, edited);
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        for settlement in &device.doc.shared_events[0].settlements {
            if let Some(id) = &settlement.tx_id {
                assert!(has_tx(device, id), "settlement names a missing transfer");
            }
        }
    }
    let by_id = |vault: &UnlockedVault| {
        let mut all = vault.doc.transactions.clone();
        all.sort_by(|first, second| first.meta().id.cmp(&second.meta().id));
        all
    };
    assert_eq!(by_id(&left), by_id(&right));
}
