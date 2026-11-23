use crate::Amount;
use crate::calendar::Calendar;
use crate::service::FlowClass;
use crate::time::secs_or_now;
use std::collections::HashMap;

use super::testing::*;
use crate::ipc_contract::{
    SettleSharedInput, SharedEventDraft, SharedParticipantDraft, SharedPurchaseDraft,
    SharedRemainderInput,
};
use crate::model::planning::{BudgetPeriod, BudgetTarget};
use crate::model::shared_event::{PriceMode, SettleDirection};
use crate::model::store::StoreKind;
use crate::service::BudgetDraft;
use crate::service::test_support::{empty_vault, income_of, store};

fn spent_in_flows(scenario: &Scenario) -> Amount {
    let cal = Calendar::new("UTC");
    crate::planning::flows::flows_between(&scenario.vault.books(), &cal, "rub", i64::MIN, i64::MAX)
        .into_iter()
        .filter(|flow| matches!(flow.class, FlowClass::Fixed | FlowClass::Daily))
        .fold(Amount::ZERO, |sum, flow| sum + flow.value)
}

fn guest(name: &str) -> SharedParticipantDraft {
    SharedParticipantDraft {
        name: name.into(),
        ..Default::default()
    }
}

fn dinner(wallet: &str, buyer: &str, total: &str) -> SharedPurchaseDraft {
    SharedPurchaseDraft {
        new_item_title: Some("Ужин".into()),
        buyer: buyer.into(),
        qty: amount("1"),
        amount: amount(total),
        currency: "rub".into(),
        price_mode: PriceMode::Total,
        store_id: Some(wallet.to_string()),
        at: Some("2026-10-03".into()),
        ..Default::default()
    }
}

#[test]
fn paid_6000_with_my_share_1500_is_an_expense_of_1500() {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("rub");
    let wallet = store(&mut vault, "Bank", StoreKind::Bank, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&wallet, rub(), "50000", "0.0109", "salary"),
    )
    .unwrap();
    let event = crate::usecases::household::shared_events::create_shared_event(
        &mut vault,
        SharedEventDraft {
            title: "Ужин".into(),
            currency: "rub".into(),
            participants: vec![guest("A"), guest("B"), guest("C")],
            category_id: Some("groceries".into()),
            ..SharedEventDraft::default()
        },
    )
    .unwrap()
    .event
    .id;
    crate::usecases::household::shared_events::add_shared_purchase(
        &mut vault,
        &event,
        dinner(&wallet, "", "6000"),
    )
    .unwrap();
    let scenario = Scenario {
        _dir: dir,
        vault,
        wallet,
        event,
        contacts: HashMap::new(),
    };
    // 6000 left the wallet, 1500 is my spending
    assert_eq!(scenario.balance(), amount("44000"));
    assert_eq!(spent_in_flows(&scenario), amount("1500"));
    let view = crate::household::shared_events::view::shared_event_view(
        &scenario.vault.doc,
        &scenario.event,
    )
    .unwrap();
    assert_eq!(view.mine.unwrap().owed_to_me, amount("4500"));
}

#[test]
fn the_skewers_count_my_share_of_every_purchase_and_nothing_twice() {
    let mut scenario = skewers_scenario();
    // my share is 960 of the meat I paid for and 100 of the coal Vasya paid for
    assert_eq!(spent_in_flows(&scenario), amount("1060"));
    // a return of money is not spending, so it changes nothing
    let wallet = scenario.wallet.clone();
    let hoisted = SettleSharedInput {
        event_id: scenario.event.clone(),
        participant_id: scenario.person("petya"),
        direction: SettleDirection::ToOrganizer,
        amount: amount("1735"),
        store_id: Some(wallet),
        at: None,
    };
    crate::usecases::household::shared_events::settle_shared(&mut scenario.vault, hoisted).unwrap();
    assert_eq!(spent_in_flows(&scenario), amount("1060"));
}

#[test]
fn a_write_off_is_my_expense_in_the_category_of_the_event() {
    let mut scenario = skewers_scenario();
    let vasya = scenario.person("vasya");
    crate::usecases::household::shared_events::settle_shared(
        &mut scenario.vault,
        SettleSharedInput {
            event_id: scenario.event.clone(),
            participant_id: vasya.clone(),
            direction: SettleDirection::ToOrganizer,
            amount: amount("1000"),
            store_id: None,
            at: None,
        },
    )
    .unwrap();
    crate::usecases::household::shared_events::write_off_shared(
        &mut scenario.vault,
        SharedRemainderInput {
            event_id: scenario.event.clone(),
            participant_id: vasya,
            amount: None,
            to: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(spent_in_flows(&scenario), amount("1060") + amount("685"));
}

#[test]
fn a_calculator_for_others_adds_no_spending_and_no_debts() {
    let mut scenario = skewers_scenario();
    let before = spent_in_flows(&scenario);
    let view = crate::usecases::household::shared_events::create_shared_event(
        &mut scenario.vault,
        SharedEventDraft {
            title: "Для других".into(),
            currency: "rub".into(),
            i_participate: false,
            organizer: "x".into(),
            participants: vec![
                SharedParticipantDraft {
                    id: Some("x".into()),
                    name: "X".into(),
                    ..Default::default()
                },
                guest("Y"),
            ],
            ..SharedEventDraft::default()
        },
    )
    .unwrap();
    crate::usecases::household::shared_events::add_shared_purchase(
        &mut scenario.vault,
        &view.event.id,
        dinner("", "x", "900"),
    )
    .unwrap();
    assert_eq!(spent_in_flows(&scenario), before);
    assert_eq!(
        crate::usecases::household::contacts::contact_balances(&scenario.vault).len(),
        3
    );
}

#[test]
fn budgets_see_my_part_not_the_whole_payment() {
    let mut scenario = skewers_scenario();
    // the payments have no category of their own: «other»
    let budget = crate::usecases::planning::budget::create_budget(
        &mut scenario.vault,
        BudgetDraft {
            active: true,
            target: BudgetTarget::Category("other".into()),
            period: BudgetPeriod::Month,
            limit: amount("100000"),
            currency: "rub".into(),
            warn: false,
        },
    )
    .unwrap();
    // 15 October 2026: the purchases are dated 3 October
    let now = secs_or_now("2026-10-15T12:00:00Z");
    let statuses =
        crate::usecases::planning::budget::budget_status(&scenario.vault.books(), now, "UTC");
    let status = statuses
        .iter()
        .find(|row| row.budget.id == budget.id)
        .unwrap();
    assert_eq!(status.spent, amount("1060"));
    let _ = &mut scenario;
}
