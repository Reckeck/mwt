use super::testing::*;
use crate::Amount;
use crate::ipc_contract::{SetSharedDueInput, SettleSharedInput, SharedRemainderInput};
use crate::ipc_contract::{SharedEventDraft, SharedParticipantDraft, SharedPurchaseDraft};
use crate::model::shared_event::{ME, SettleDirection};
use crate::model::shared_event::{PriceMode, SharedEventStatus};
use crate::model::transaction::{DebtEffect, Transaction, TransferTx};

fn settle(
    scenario: &Scenario,
    who: &str,
    direction: SettleDirection,
    raw: &str,
    store: Option<&str>,
) -> SettleSharedInput {
    SettleSharedInput {
        event_id: scenario.event.clone(),
        participant_id: who.into(),
        direction,
        amount: amount(raw),
        store_id: store.map(str::to_string),
        at: None,
    }
}

/// The debt transfer an event settlement made.
fn last_transfer(scenario: &Scenario) -> TransferTx {
    let linked: Vec<String> =
        crate::household::shared_events::view::shared_tx_links(&scenario.vault.doc)
            .into_iter()
            .filter(|link| link.link.settlement_id.is_some())
            .map(|link| link.tx_id)
            .collect();
    scenario
        .vault
        .doc
        .transactions
        .iter()
        .find_map(|tx| match tx {
            Transaction::Transfer(inner) if linked.contains(&inner.meta.id) => Some(inner.clone()),
            _ => None,
        })
        .unwrap()
}

fn debt_with(scenario: &Scenario, contact: &str) -> Amount {
    crate::usecases::household::contacts::contact_balances(&scenario.vault)
        .into_iter()
        .filter(|row| row.contact_id == contact)
        .fold(Amount::ZERO, |sum, row| sum + row.receivable - row.payable)
}

#[test]
fn debts_appear_at_once_in_contact_cards_and_go_out_when_returned() {
    let mut scenario = skewers_scenario();
    let petya = scenario.contact("petya");
    let vasya = scenario.contact("vasya");
    assert_eq!(debt_with(&scenario, &petya), amount("1735"));
    assert_eq!(debt_with(&scenario, &vasya), amount("1685"));
    // the guest is nowhere in the contacts
    assert_eq!(
        crate::usecases::household::contacts::contact_balances(&scenario.vault).len(),
        3,
        "Petya, Vasya and Anya only"
    );
    let cards =
        crate::household::shared_events::view::contact_shared_events(&scenario.vault.doc, &petya);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].amount, amount("1735"));

    let person = scenario.person("petya");
    let wallet = scenario.wallet.clone();
    let hoisted = settle(
        &scenario,
        &person,
        SettleDirection::ToOrganizer,
        "1735",
        Some(&wallet),
    );
    let view =
        crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted)
            .unwrap();
    assert_eq!(view.status, SharedEventStatus::Settling);
    assert_eq!(debt_with(&scenario, &petya), amount("0"));
    assert!(
        crate::household::shared_events::view::contact_shared_events(&scenario.vault.doc, &petya)
            .is_empty()
    );
    // the money is in my wallet, and the operation is a Collect from Petya
    assert_eq!(scenario.balance(), amount("43400") + amount("1735"));
    let transfer = last_transfer(&scenario);
    assert_eq!(transfer.debt, Some(DebtEffect::Collect));
    assert_eq!(transfer.amount, amount("1735"));
}

#[test]
fn a_partial_return_leaves_the_rest_and_too_much_is_refused() {
    let mut scenario = skewers_scenario();
    let vasya = scenario.person("vasya");
    let wallet = scenario.wallet.clone();
    let hoisted = settle(
        &scenario,
        &vasya,
        SettleDirection::ToOrganizer,
        "1000",
        Some(&wallet),
    );
    crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted).unwrap();
    assert_eq!(
        debt_with(&scenario, &scenario.contact("vasya")),
        amount("685")
    );
    let hoisted = settle(
        &scenario,
        &vasya,
        SettleDirection::ToOrganizer,
        "686",
        Some(&wallet),
    );
    let too_much =
        crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted);
    assert!(too_much.is_err());
    // giving money to someone who owes me makes no sense either
    let hoisted = settle(
        &scenario,
        &vasya,
        SettleDirection::FromOrganizer,
        "10",
        None,
    );
    let backwards =
        crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted);
    assert!(backwards.is_err());
}

#[test]
fn a_set_off_changes_the_balance_without_an_operation() {
    let mut scenario = skewers_scenario();
    let anya = scenario.person("anya");
    let before = scenario.vault.doc.transactions.len();
    let hoisted = settle(&scenario, &anya, SettleDirection::ToOrganizer, "1060", None);
    crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted).unwrap();
    assert_eq!(scenario.vault.doc.transactions.len(), before);
    assert_eq!(debt_with(&scenario, &scenario.contact("anya")), amount("0"));
}

#[test]
fn a_guest_settles_without_an_operation_only() {
    let mut scenario = skewers_scenario();
    let olya = scenario.person("olya");
    let wallet = scenario.wallet.clone();
    let hoisted = settle(
        &scenario,
        &olya,
        SettleDirection::ToOrganizer,
        "1060",
        Some(&wallet),
    );
    let refused =
        crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted);
    assert!(refused.is_err());
    let hoisted = settle(&scenario, &olya, SettleDirection::ToOrganizer, "1060", None);
    let view =
        crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted)
            .unwrap();
    let guest = view.people.iter().find(|row| row.id == olya).unwrap();
    assert!(guest.saldo.is_zero());
}

#[test]
fn writing_off_the_rest_closes_the_debt_and_is_my_position() {
    let mut scenario = skewers_scenario();
    let vasya = scenario.person("vasya");
    let wallet = scenario.wallet.clone();
    let hoisted = settle(
        &scenario,
        &vasya,
        SettleDirection::ToOrganizer,
        "1000",
        Some(&wallet),
    );
    crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted).unwrap();
    let rest = |amount: Option<Amount>| SharedRemainderInput {
        event_id: scenario.event.clone(),
        participant_id: vasya.clone(),
        amount,
        to: Vec::new(),
    };
    let view = crate::usecases::household::shared_events::write_off_shared(
        &mut scenario.vault,
        rest(None),
    )
    .unwrap();
    assert_eq!(
        debt_with(&scenario, &scenario.contact("vasya")),
        amount("0")
    );
    let mine = view.mine.unwrap();
    assert_eq!(mine.written_off, amount("685"));
    // nothing to write off any more
    assert!(
        crate::usecases::household::shared_events::write_off_shared(
            &mut scenario.vault,
            rest(None)
        )
        .is_err()
    );
}

#[test]
fn putting_the_rest_on_the_others_shifts_their_shares() {
    let mut scenario = skewers_scenario();
    let vasya = scenario.person("vasya");
    let wallet = scenario.wallet.clone();
    let hoisted = settle(
        &scenario,
        &vasya,
        SettleDirection::ToOrganizer,
        "1000",
        Some(&wallet),
    );
    crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted).unwrap();
    let view = crate::usecases::household::shared_events::reassign_shared(
        &mut scenario.vault,
        SharedRemainderInput {
            event_id: scenario.event.clone(),
            participant_id: vasya.clone(),
            amount: None,
            to: Vec::new(),
        },
    )
    .unwrap();
    // 685 over Me, Petya, Anya, Olya: 171.25 each
    let row = |id: &str| view.people.iter().find(|row| row.id == id).unwrap();
    assert_eq!(row(&vasya).share, amount("1500"));
    assert!(row(&vasya).saldo.is_zero());
    assert_eq!(row(ME).share, amount("1231.25"));
    assert_eq!(
        debt_with(&scenario, &scenario.contact("petya")),
        amount("1906.25")
    );
    let total = view
        .people
        .iter()
        .fold(Amount::ZERO, |sum, row| sum + row.share);
    assert_eq!(total, amount("7100"));
}

#[test]
fn a_settlement_can_be_taken_back_with_its_operation() {
    let mut scenario = skewers_scenario();
    let petya = scenario.person("petya");
    let wallet = scenario.wallet.clone();
    let hoisted = settle(
        &scenario,
        &petya,
        SettleDirection::ToOrganizer,
        "500",
        Some(&wallet),
    );
    let view =
        crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted)
            .unwrap();
    let settlement = view.event.settlements[0].clone();
    let balance = scenario.balance();
    crate::usecases::household::shared_events::delete_shared_settlement(
        &mut scenario.vault,
        &scenario.event,
        &settlement.id,
    )
    .unwrap();
    assert_eq!(scenario.balance(), balance - amount("500"));
    assert_eq!(
        debt_with(&scenario, &scenario.contact("petya")),
        amount("1735")
    );
}

#[test]
fn when_someone_else_organizes_one_repay_settles_my_share() {
    let mut scenario = gift_scenario();
    let anya = scenario.contact("anya");
    // I owe Anya my 1850: a negative debt of hers
    assert_eq!(debt_with(&scenario, &anya), amount("-1850"));
    let view = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap();
    let mine = view.mine.clone().unwrap();
    assert_eq!(mine.i_owe, amount("1850"));
    assert_eq!(
        mine.pay_to.as_deref(),
        Some(scenario.person("anya").as_str())
    );
    let wallet = scenario.wallet.clone();
    let transfers = |scenario: &Scenario| {
        scenario
            .vault
            .doc
            .transactions
            .iter()
            .filter(|tx| matches!(tx, Transaction::Transfer(_)))
            .count()
    };
    let before = transfers(&scenario);
    let hoisted = settle(
        &scenario,
        ME,
        SettleDirection::ToOrganizer,
        "1850",
        Some(&wallet),
    );
    let view =
        crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted)
            .unwrap();
    assert_eq!(transfers(&scenario), before + 1);
    let repay = last_transfer(&scenario);
    assert_eq!(repay.debt, Some(DebtEffect::Repay));
    assert_eq!(repay.amount, amount("1850"));
    assert_eq!(debt_with(&scenario, &anya), amount("0"));
    assert!(view.mine.unwrap().i_owe.is_zero());
}

#[test]
fn a_calculator_for_others_touches_no_wallet_and_no_contact() {
    let mut scenario = gift_scenario();
    let draft = SharedEventDraft {
        title: "Для друзей".into(),
        currency: "rub".into(),
        i_participate: false,
        organizer: "pa".into(),
        participants: vec![
            SharedParticipantDraft {
                id: Some("pa".into()),
                contact_id: Some(scenario.contact("anya")),
                ..Default::default()
            },
            SharedParticipantDraft {
                id: Some("pb".into()),
                contact_id: Some(scenario.contact("petya")),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let id =
        crate::usecases::household::shared_events::create_shared_event(&mut scenario.vault, draft)
            .unwrap()
            .event
            .id;
    let cake = |buyer: &str| SharedPurchaseDraft {
        new_item_title: Some("Торт".into()),
        buyer: buyer.into(),
        qty: amount("1"),
        amount: amount("900"),
        price_mode: PriceMode::Total,
        store_id: Some(scenario.wallet.clone()),
        ..Default::default()
    };
    // I cannot buy for it, and it creates no debts with my contacts
    assert!(
        crate::usecases::household::shared_events::add_shared_purchase(
            &mut scenario.vault,
            &id,
            cake("")
        )
        .is_err()
    );
    let view = crate::usecases::household::shared_events::add_shared_purchase(
        &mut scenario.vault,
        &id,
        cake("pa"),
    )
    .unwrap();
    assert!(view.mine.is_none());
    assert_eq!(
        view.people.iter().find(|row| row.id == "pb").unwrap().saldo,
        amount("450")
    );
    let balance = scenario.balance();
    let money = SettleSharedInput {
        event_id: id.clone(),
        participant_id: "pb".into(),
        direction: SettleDirection::ToOrganizer,
        amount: amount("450"),
        store_id: Some(scenario.wallet.clone()),
        at: None,
    };
    // settling is a set-off only
    assert!(
        crate::usecases::household::shared_events::settle_shared(
            &mut scenario.vault,
            money.clone()
        )
        .is_err()
    );
    crate::usecases::household::shared_events::settle_shared(
        &mut scenario.vault,
        SettleSharedInput {
            store_id: None,
            ..money
        },
    )
    .unwrap();
    assert_eq!(scenario.balance(), balance);
    let petya = scenario.contact("petya");
    assert!(
        crate::household::shared_events::view::contact_shared_events(&scenario.vault.doc, &petya)
            .iter()
            .all(|row| row.event_id != id)
    );
}

#[test]
fn a_term_can_be_set_for_a_contact_debt() {
    let mut scenario = skewers_scenario();
    let petya = scenario.person("petya");
    let terms = crate::usecases::household::shared_events::set_shared_due(
        &mut scenario.vault,
        SetSharedDueInput {
            event_id: scenario.event.clone(),
            participant_id: petya.clone(),
            due_at: Some("2026-12-01".into()),
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(terms.asset, "RUB");
    assert_eq!(terms.due_at.as_deref(), Some("2026-12-01"));
    let view = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap();
    let row = view.people.iter().find(|row| row.id == petya).unwrap();
    assert_eq!(row.due_at.as_deref(), Some("2026-12-01"));
    // a guest has no contact to hang a term on
    let olya = scenario.person("olya");
    assert!(
        crate::usecases::household::shared_events::set_shared_due(
            &mut scenario.vault,
            SetSharedDueInput {
                event_id: scenario.event.clone(),
                participant_id: olya,
                due_at: Some("2026-12-01".into()),
            }
        )
        .is_err()
    );
}

/// The risk register asks for a bench on a big vault: 500 events with
/// debts must not slow the contact balances noticeably.
#[test]
fn five_hundred_events_keep_the_contact_balances_fast() {
    let mut scenario = skewers_scenario();
    let template = scenario.vault.doc.shared_events[0].clone();
    for index in 0..500 {
        let mut copy = template.clone();
        copy.id = format!("bulk-{index}");
        for item in &mut copy.items {
            for purchase in &mut item.purchases {
                purchase.tx_id = None;
                purchase.tx_owned = false;
            }
        }
        scenario.vault.doc.shared_events.push(copy);
    }
    let started = std::time::Instant::now();
    let balances = crate::usecases::household::contacts::contact_balances(&scenario.vault);
    let elapsed = started.elapsed();
    assert_eq!(balances.len(), 3);
    // 501 events x 1 735 for Petya
    let petya = balances
        .iter()
        .find(|row| row.contact_id == scenario.contact("petya"))
        .unwrap();
    assert_eq!(petya.receivable, amount("1735") * amount("501"));
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "took {elapsed:?}"
    );
}

#[test]
fn the_organizer_cannot_settle_with_themselves() {
    let mut scenario = skewers_scenario();
    let hoisted = settle(&scenario, ME, SettleDirection::ToOrganizer, "10", None);
    let refused =
        crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted);
    assert!(refused.is_err());
}
