use super::testing::*;
use crate::Amount;
use crate::ipc_contract::{SetShareOverrideInput, SettleSharedInput, SharedRemainderInput};
use crate::model::shared_event::ME;
use crate::model::shared_event::{Purchase, RemovedWithPayment, SettleDirection};
use crate::model::transaction::Transaction;
use crate::service::Session;

fn settle_petya(scenario: &mut Scenario, raw: &str, at: Option<&str>) -> String {
    let wallet = scenario.wallet.clone();
    crate::usecases::household::shared_events::settle_shared(
        &mut scenario.vault,
        SettleSharedInput {
            event_id: scenario.event.clone(),
            participant_id: "petya".into(),
            direction: SettleDirection::ToOrganizer,
            amount: amount(raw),
            store_id: Some(wallet),
            at: at.map(str::to_string),
        },
    )
    .unwrap();
    crate::household::shared_events::view::shared_tx_links(&scenario.vault.doc)
        .into_iter()
        .find(|link| link.link.settlement_id.is_some())
        .unwrap()
        .tx_id
}

fn petya_debt(scenario: &Scenario, until: i64) -> Amount {
    let petya = scenario.contact("petya");
    crate::household::contacts::contact_balances_until(&scenario.vault.books(), until)
        .into_iter()
        .filter(|row| row.contact_id == petya)
        .fold(Amount::ZERO, |sum, row| sum + row.receivable - row.payable)
}

#[test]
fn deleting_a_linked_payment_in_the_ledger_removes_the_purchase() {
    let mut scenario = skewers_scenario();
    let link = crate::household::shared_events::view::shared_tx_links(&scenario.vault.doc)
        .into_iter()
        .find(|link| link.link.purchase_id.is_some())
        .unwrap();
    crate::usecases::money::transactions::delete_transaction(&mut scenario.vault, &link.tx_id)
        .unwrap();
    let view = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap();
    // the meat (4 800) was bought with that payment or the beer was; either way one is gone
    assert!(view.totals.spent < amount("7100"));
    assert!(
        crate::household::shared_events::view::shared_tx_links(&scenario.vault.doc)
            .iter()
            .all(|other| other.tx_id != link.tx_id)
    );
}

#[test]
fn deleting_a_settlement_operation_reopens_the_debt() {
    let mut scenario = skewers_scenario();
    let tx = settle_petya(&mut scenario, "1735", None);
    assert_eq!(petya_debt(&scenario, i64::MAX), amount("0"));
    crate::usecases::money::bulk::delete_transactions(
        &mut scenario.vault,
        std::slice::from_ref(&tx),
    )
    .unwrap();
    assert_eq!(petya_debt(&scenario, i64::MAX), amount("1735"));
    let view = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap();
    assert!(view.event.settlements.is_empty());
}

/// The payment and the purchase of one link.
fn linked_purchase(scenario: &Scenario) -> (String, String) {
    let link = crate::household::shared_events::view::shared_tx_links(&scenario.vault.doc)
        .into_iter()
        .find(|link| link.link.purchase_id.is_some())
        .unwrap();
    (link.tx_id, link.link.purchase_id.unwrap())
}

#[test]
fn undoing_the_deletion_of_a_payment_brings_its_purchase_back() {
    let mut scenario = skewers_scenario();
    let (tx_id, _) = linked_purchase(&scenario);
    let spent = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap()
    .totals
    .spent;
    let deleted = crate::usecases::money::bulk::delete_transactions(
        &mut scenario.vault,
        std::slice::from_ref(&tx_id),
    )
    .unwrap();
    crate::usecases::money::bulk::restore_transactions(&mut scenario.vault, deleted).unwrap();
    let view = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap();
    assert_eq!(view.totals.spent, spent);
    assert!(view.event.removed_with_payment.is_empty());
}

#[test]
fn undoing_the_deletion_of_a_settlement_operation_settles_again() {
    let mut scenario = skewers_scenario();
    let tx = settle_petya(&mut scenario, "1735", None);
    let deleted = crate::usecases::money::bulk::delete_transactions(
        &mut scenario.vault,
        std::slice::from_ref(&tx),
    )
    .unwrap();
    crate::usecases::money::bulk::restore_transactions(&mut scenario.vault, deleted).unwrap();
    assert_eq!(petya_debt(&scenario, i64::MAX), amount("0"));
}

#[test]
fn a_purchase_taken_out_by_hand_leaves_nothing_to_bring_back() {
    let mut scenario = skewers_scenario();
    let (_, purchase_id) = linked_purchase(&scenario);
    let event = scenario.event.clone();
    crate::usecases::household::shared_events::delete_shared_purchase(
        &mut scenario.vault,
        &event,
        &purchase_id,
    )
    .unwrap();
    let view = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap();
    assert!(view.event.removed_with_payment.is_empty());
}

#[test]
fn a_payment_that_found_another_place_is_not_linked_twice() {
    let mut scenario = skewers_scenario();
    let (tx_id, _) = linked_purchase(&scenario);
    let position = |vault: &Session| {
        vault
            .doc
            .shared_events
            .iter()
            .position(|event| event.id == scenario.event)
            .unwrap()
    };
    let index = position(&scenario.vault);
    let event = &mut scenario.vault.doc.shared_events[index];
    event.removed_with_payment.push(RemovedWithPayment {
        id: "stale".into(),
        tx_id: tx_id.clone(),
        item_id: Some(event.items[0].id.clone()),
        purchase: Some(Purchase {
            id: "stale".into(),
            tx_id: Some(tx_id.clone()),
            ..Purchase::default()
        }),
        settlement: None,
    });
    crate::usecases::sync::repair::restore_removed_with_payment(&mut scenario.vault);
    let event = &scenario.vault.doc.shared_events[index];
    assert!(event.removed_with_payment.is_empty());
    let copies = event
        .items
        .iter()
        .flat_map(|item| &item.purchases)
        .filter(|purchase| purchase.tx_id.as_deref() == Some(tx_id.as_str()))
        .count();
    assert_eq!(copies, 1);
}

fn edited(scenario: &Scenario, tx_id: &str, change: impl FnOnce(&mut Transaction)) -> Transaction {
    let mut tx = scenario
        .vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == tx_id)
        .unwrap()
        .clone();
    change(&mut tx);
    tx
}

#[test]
fn editing_the_amount_and_date_of_a_linked_operation_follows_into_the_event() {
    let mut scenario = skewers_scenario();
    let purchase_tx = crate::household::shared_events::view::shared_tx_links(&scenario.vault.doc)
        .into_iter()
        .find(|link| link.link.purchase_id.is_some())
        .unwrap()
        .tx_id;
    let tx = edited(&scenario, &purchase_tx, |tx| {
        if let Transaction::Payment(inner) = tx {
            inner.amount = amount("5000");
            inner.meta.timestamp = "2026-10-04T10:00:00Z".into();
        }
    });
    crate::usecases::money::transactions::update_transaction(&mut scenario.vault, &purchase_tx, tx)
        .unwrap();
    let event = scenario.vault.find_shared_event(&scenario.event).unwrap();
    let purchase = event
        .items
        .iter()
        .flat_map(|item| &item.purchases)
        .find(|purchase| purchase.tx_id.as_deref() == Some(purchase_tx.as_str()))
        .unwrap();
    assert_eq!(purchase.total(), amount("5000"));
    assert_eq!(purchase.at, "2026-10-04T10:00:00Z");

    // a settlement edit
    let settle_tx = settle_petya(&mut scenario, "1000", None);
    let tx = edited(&scenario, &settle_tx, |tx| {
        if let Transaction::Transfer(inner) = tx {
            inner.amount = amount("1200");
        }
    });
    crate::usecases::money::transactions::update_transaction(&mut scenario.vault, &settle_tx, tx)
        .unwrap();
    let view = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap();
    assert_eq!(view.event.settlements[0].amount, amount("1200"));
}

#[test]
fn the_event_refuses_what_it_cannot_follow() {
    let mut scenario = skewers_scenario();
    let purchase_tx = crate::household::shared_events::view::shared_tx_links(&scenario.vault.doc)
        .into_iter()
        .find(|link| link.link.purchase_id.is_some())
        .unwrap()
        .tx_id;
    // another currency
    let tx = edited(&scenario, &purchase_tx, |tx| {
        if let Transaction::Payment(inner) = tx {
            inner.asset = crate::model::store::AssetRef::fiat("USD", "USD");
        }
    });
    assert!(
        crate::usecases::money::transactions::update_transaction(
            &mut scenario.vault,
            &purchase_tx,
            tx
        )
        .is_err()
    );
    // another kind
    let tx = edited(&scenario, &purchase_tx, |tx| {
        if let Transaction::Payment(inner) = tx {
            *tx = Transaction::Topup(crate::model::transaction::TopupTx {
                meta: inner.meta.clone(),
                store_id: inner.store_id.clone(),
                asset: inner.asset.clone(),
                amount: inner.amount,
                fee: None,
                from: None,
                details: Default::default(),
                price: Default::default(),
            });
        }
    });
    assert!(
        crate::usecases::money::transactions::update_transaction(
            &mut scenario.vault,
            &purchase_tx,
            tx
        )
        .is_err()
    );
    // a settlement above what is owed
    let settle_tx = settle_petya(&mut scenario, "1000", None);
    let tx = edited(&scenario, &settle_tx, |tx| {
        if let Transaction::Transfer(inner) = tx {
            inner.amount = amount("1736");
        }
    });
    assert!(
        crate::usecases::money::transactions::update_transaction(
            &mut scenario.vault,
            &settle_tx,
            tx
        )
        .is_err()
    );
    // nothing changed
    let view = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap();
    assert_eq!(view.event.settlements[0].amount, amount("1000"));
    assert_eq!(view.totals.spent, amount("7100"));
}

#[test]
fn event_debts_are_dated_and_event_transfers_are_never_counted_twice() {
    let mut scenario = skewers_scenario();
    settle_petya(&mut scenario, "1735", Some("2026-10-05"));
    let before = crate::time::secs_or_now("2026-10-04T00:00:00Z");
    let after = crate::time::secs_or_now("2026-10-06T00:00:00Z");
    // the purchases are dated 3 October: Petya owes 1 735 on the 4th, nothing on the 6th
    assert_eq!(petya_debt(&scenario, before), amount("1735"));
    assert_eq!(petya_debt(&scenario, after), amount("0"));
    assert_eq!(petya_debt(&scenario, i64::MAX), amount("0"));
}

#[test]
fn deleting_the_event_turns_its_transfers_into_ordinary_debt_transfers() {
    let mut scenario = skewers_scenario();
    settle_petya(&mut scenario, "1735", None);
    crate::usecases::household::shared_events::delete_shared_event(
        &mut scenario.vault,
        &scenario.event.clone(),
    )
    .unwrap();
    // a bare Collect of 1 735 with nothing lent before: the debt goes negative
    assert_eq!(petya_debt(&scenario, i64::MAX), amount("-1735"));
    assert!(crate::household::shared_events::view::shared_tx_links(&scenario.vault.doc).is_empty());
}

#[test]
fn editing_an_archived_event_reopens_it() {
    let mut scenario = skewers_scenario();
    let id = scenario.event.clone();
    // settle everything by set-off, then archive
    for (who, raw) in [
        ("petya", "1735"),
        ("vasya", "1685"),
        ("anya", "1060"),
        ("olya", "1060"),
    ] {
        crate::usecases::household::shared_events::settle_shared(
            &mut scenario.vault,
            SettleSharedInput {
                event_id: id.clone(),
                participant_id: who.into(),
                direction: SettleDirection::ToOrganizer,
                amount: amount(raw),
                store_id: None,
                at: None,
            },
        )
        .unwrap();
    }
    crate::usecases::household::shared_events::archive_shared_event(&mut scenario.vault, &id, true)
        .unwrap();
    assert!(scenario.vault.find_shared_event(&id).unwrap().archived);
    crate::usecases::household::shared_events::add_shared_item(
        &mut scenario.vault,
        &id,
        crate::ipc_contract::SharedItemDraft {
            title: "Лук".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!scenario.vault.find_shared_event(&id).unwrap().archived);
}

#[test]
fn a_participant_with_operations_keeps_their_contact_and_a_lone_sharer_stays() {
    let mut scenario = skewers_scenario();
    let tx = settle_petya(&mut scenario, "100", None);
    assert!(!tx.is_empty());
    let other = scenario.contact("anya");
    let relink = crate::usecases::household::shared_events::update_shared_participant(
        &mut scenario.vault,
        &scenario.event.clone(),
        "petya",
        crate::ipc_contract::SharedParticipantDraft {
            contact_id: Some(other),
            ..Default::default()
        },
    );
    assert!(relink.is_err());
    // Olya is the only one sharing the napkins
    let id = scenario.event.clone();
    let napkins = crate::usecases::household::shared_events::add_shared_item(
        &mut scenario.vault,
        &id,
        crate::ipc_contract::SharedItemDraft {
            title: "Салфетки".into(),
            ..Default::default()
        },
    )
    .unwrap()
    .event
    .items
    .last()
    .unwrap()
    .id
    .clone();
    crate::usecases::household::shared_events::set_shared_item_split(
        &mut scenario.vault,
        crate::ipc_contract::SetSharedItemSplitInput {
            event_id: id.clone(),
            item_id: napkins,
            split: crate::model::shared_event::ItemSplit::Equal {
                among: vec!["olya".into()],
            },
        },
    )
    .unwrap();
    let err = crate::usecases::household::shared_events::remove_shared_participant(
        &mut scenario.vault,
        &id,
        "olya",
    )
    .unwrap_err();
    assert!(err.to_string().contains("Салфетки"), "{err}");
}

#[test]
fn fixed_totals_are_checked_when_set_and_flagged_when_they_stop_fitting() {
    let mut scenario = skewers_scenario();
    let id = scenario.event.clone();
    let set = |scenario: &mut Scenario, who: &str, total: Option<&str>| {
        crate::usecases::household::shared_events::set_shared_share_override(
            &mut scenario.vault,
            SetShareOverrideInput {
                event_id: id.clone(),
                participant_id: who.into(),
                total: total.map(amount),
            },
        )
    };
    assert!(set(&mut scenario, "petya", Some("9000")).is_err());
    assert!(set(&mut scenario, "petya", Some("1000")).is_ok());
    // everybody fixed to a sum that is not what was bought
    for who in [ME, "vasya", "anya"] {
        assert!(set(&mut scenario, who, Some("100")).is_ok());
    }
    assert!(set(&mut scenario, "olya", Some("100")).is_err());
    // a purchase removed later makes the totals not fit
    assert!(set(&mut scenario, "olya", Some("5800")).is_ok());
    let tx = crate::household::shared_events::view::shared_tx_links(&scenario.vault.doc)
        .into_iter()
        .find(|link| link.link.purchase_id.is_some())
        .unwrap()
        .tx_id;
    crate::usecases::money::transactions::delete_transaction(&mut scenario.vault, &tx).unwrap();
    let view =
        crate::household::shared_events::view::shared_event_view(&scenario.vault.doc, &id).unwrap();
    assert!(view.overrides_invalid);
}

#[test]
fn passing_a_rest_on_never_lands_on_an_organizer_who_is_out() {
    let mut scenario = skewers_scenario();
    let id = scenario.event.clone();
    scenario
        .vault
        .find_shared_event_mut(&id)
        .unwrap()
        .i_participate = false;
    // the purchases of mine now have no place; only the debt rule matters here
    let rest = crate::usecases::household::shared_events::reassign_shared(
        &mut scenario.vault,
        SharedRemainderInput {
            event_id: id.clone(),
            participant_id: "vasya".into(),
            amount: Some(amount("100")),
            to: vec![ME.into()],
        },
    );
    assert!(rest.is_err());
    let view = crate::usecases::household::shared_events::reassign_shared(
        &mut scenario.vault,
        SharedRemainderInput {
            event_id: id.clone(),
            participant_id: "vasya".into(),
            amount: Some(amount("100")),
            to: Vec::new(),
        },
    )
    .unwrap();
    assert!(view.event.settlements[0].to.iter().all(|who| who != ME));
}
