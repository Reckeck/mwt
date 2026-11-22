//! Tests of the split: the old inline tests of `split.rs`, names and bodies
//! unchanged, on the fixtures of the "skewers" board.

use super::split::*;
use mwt_base::Amount;
use mwt_model::{ItemSplit, ME, Purchase, SettlementKind, SharedEvent, SharedEventStatus};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

mod fixtures {
    use super::*;
    use mwt_model::{EventItem, ItemPlan, Participant, PriceMode};

    pub(super) fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    pub(super) fn person(id: &str, name: &str) -> Participant {
        Participant {
            id: id.into(),
            name: name.into(),
            ..Participant::default()
        }
    }

    pub(super) fn purchase(
        id: &str,
        buyer: &str,
        qty: &str,
        total: &str,
        currency: &str,
    ) -> Purchase {
        Purchase {
            id: id.into(),
            buyer: buyer.into(),
            qty: amount(qty),
            amount: amount(total),
            currency: currency.into(),
            price_mode: PriceMode::Total,
            rate_to_event: one(),
            at: "2026-11-02T12:00:00Z".into(),
            ..Purchase::default()
        }
    }

    pub(super) fn item(id: &str, title: &str, plan: Option<(&str, &str)>) -> EventItem {
        EventItem {
            id: id.into(),
            title: title.into(),
            unit: "шт".into(),
            plan: plan.map(|(qty, total)| ItemPlan {
                qty: amount(qty),
                price: amount(total),
                price_mode: PriceMode::Total,
            }),
            ..EventItem::default()
        }
    }

    /// The "Barbecue" board: me, Petya, Vasya, Anya and the guest Olya, in rubles.
    pub(super) fn skewers() -> SharedEvent {
        let mut meat = item("meat", "Мясо", Some(("4", "4800")));
        meat.purchases = vec![purchase("p-meat", ME, "4", "4800", "rub")];
        let mut beer = item("beer", "Пиво", Some(("6", "1200")));
        beer.purchases = vec![purchase("p-beer", ME, "8", "1800", "rub")];
        beer.split = ItemSplit::ByUnits {
            units: BTreeMap::from([
                ("petya".to_string(), amount("3")),
                ("vasya".to_string(), amount("5")),
            ]),
        };
        let mut coal = item("coal", "Уголь", None);
        coal.purchases = vec![purchase("p-coal", "vasya", "1", "500", "rub")];
        let veg = item("veg", "Овощи", Some(("3", "900")));
        let pita = item("pita", "Лаваш", Some(("4", "300")));
        let sauce = item("sauce", "Соус", Some(("2", "400")));
        let juice = item("juice", "Сок", Some(("6", "1800")));
        SharedEvent {
            id: "skewers".into(),
            title: "Шашлыки".into(),
            currency: "rub".into(),
            rounding_step: amount("1"),
            organizer: ME.into(),
            i_participate: true,
            participants: vec![
                person("petya", "Петя"),
                person("vasya", "Вася"),
                person("anya", "Аня"),
                person("olya", "Оля"),
            ],
            items: vec![meat, beer, veg, pita, sauce, juice, coal],
            ..SharedEvent::default()
        }
    }
}

use fixtures::*;
use mwt_model::{EventItem, PriceMode, Settlement, ShareOverride};

fn share_of(split: &Split, id: &str) -> Amount {
    split.person(id).unwrap().share
}

#[test]
fn the_skewers_board_adds_up_to_the_cent() {
    let event = skewers();
    let split = compute(&event);
    let shares = split
        .people
        .iter()
        .fold(Amount::ZERO, |sum, row| sum + row.share);
    assert_eq!(shares, split.spent);
    let saldo_sum = split
        .people
        .iter()
        .fold(Amount::ZERO, |sum, row| sum + row.saldo);
    assert!(saldo_sum.is_zero());
    assert_eq!(status(&event, &split), SharedEventStatus::InProgress);
    // The whole board, pinned: totals, every person's share, paid and saldo
    // (me paid 6600 for a share of 1060), every item's cost against its plan
    // (beer: 8 bought against 6 planned, 600 over) and the purchase lines.
    insta::assert_debug_snapshot!(split);
}

#[test]
fn nothing_bought_is_planning_and_has_no_debts() {
    let mut event = skewers();
    for item in &mut event.items {
        item.purchases.clear();
    }
    let split = compute(&event);
    assert_eq!(status(&event, &split), SharedEventStatus::Planning);
    assert!(split.all_settled());
    assert_eq!(split.planned, amount("9400"));
}

#[test]
fn equal_split_rounds_to_the_step_and_the_buyer_takes_the_leftover() {
    let mut event = skewers();
    event.items = vec![{
        let mut bill = item("bill", "Счёт", None);
        bill.purchases = vec![purchase("p", "vasya", "1", "100", "rub")];
        bill
    }];
    event.participants = vec![person("vasya", "Вася"), person("petya", "Петя")];
    event.i_participate = true;
    // 100 / 3 = 33.33 -> 33 each at step 1; the leftover 1 goes to the organizer (me)
    let split = compute(&event);
    assert_eq!(share_of(&split, ME), amount("34"));
    assert_eq!(share_of(&split, "petya"), amount("33"));
    assert_eq!(share_of(&split, "vasya"), amount("33"));
    // at step 50 the parts round to 50 and 0; the sum still holds
    event.rounding_step = amount("50");
    let coarse = compute(&event);
    let total = coarse
        .people
        .iter()
        .fold(Amount::ZERO, |sum, row| sum + row.share);
    assert_eq!(total, amount("100"));
    // cents: 100 / 3 at step 0.01
    event.rounding_step = amount("0.01");
    let fine = compute(&event);
    assert_eq!(share_of(&fine, ME), amount("33.33"));
    assert_eq!(share_of(&fine, "vasya"), amount("33.34"));
}

#[test]
fn an_item_can_be_narrowed_to_some_people() {
    let mut event = skewers();
    event.items = vec![{
        let mut beer = item("beer", "Пиво", None);
        beer.purchases = vec![purchase("p", ME, "6", "600", "rub")];
        beer.split = ItemSplit::Equal {
            among: vec!["petya".into(), "vasya".into()],
        };
        beer
    }];
    let split = compute(&event);
    assert_eq!(share_of(&split, "petya"), amount("300"));
    assert_eq!(share_of(&split, "vasya"), amount("300"));
    assert_eq!(share_of(&split, ME), amount("0"));
    assert_eq!(split.person(ME).unwrap().saldo, amount("-600"));
}

#[test]
fn two_currencies_are_settled_at_the_rate_stored_on_the_purchase() {
    let mut event = skewers();
    let mut tickets = item("tickets", "Билеты", None);
    let mut abroad = purchase("p-usd", ME, "1", "10", "usd");
    abroad.rate_to_event = amount("91.37");
    tickets.purchases = vec![abroad, purchase("p-rub", "vasya", "1", "500", "rub")];
    event.items = vec![tickets];
    event.participants = vec![person("vasya", "Вася")];
    event.rounding_step = amount("0.01");
    let split = compute(&event);
    // 10 USD at 91.37 = 913.70 plus 500 RUB
    assert_eq!(split.spent, amount("1413.70"));
    assert_eq!(share_of(&split, ME), amount("706.85"));
    assert_eq!(share_of(&split, "vasya"), amount("706.85"));
    assert_eq!(split.person(ME).unwrap().paid, amount("913.70"));
    // changing the rate later does not rewrite what was bought: only the
    // stored rate counts, so the same event gives the same numbers
    assert_eq!(compute(&event), split);
}

#[test]
fn a_hand_set_total_moves_the_rest_to_the_others() {
    let mut event = skewers();
    event.adjustments = vec![ShareOverride {
        participant_id: "anya".into(),
        total: amount("100"),
    }];
    let split = compute(&event);
    assert_eq!(share_of(&split, "anya"), amount("100"));
    let total = split
        .people
        .iter()
        .fold(Amount::ZERO, |sum, row| sum + row.share);
    assert_eq!(total, split.spent);
    // the others keep their proportions: Vasya still pays the most
    assert!(share_of(&split, "vasya") > share_of(&split, "petya"));
    assert!(share_of(&split, "petya") > share_of(&split, ME));
}

#[test]
fn a_rest_put_on_the_others_changes_the_shares_and_keeps_the_sum() {
    let mut event = skewers();
    event.settlements.push(Settlement {
        id: "s".into(),
        kind: SettlementKind::Reassign,
        participant_id: "vasya".into(),
        amount: amount("685"),
        to: vec![ME.into(), "petya".into(), "anya".into(), "olya".into()],
        ..Settlement::default()
    });
    let split = compute(&event);
    // 685 over four: 171.25 each
    assert_eq!(share_of(&split, "vasya"), amount("1500"));
    assert_eq!(share_of(&split, ME), amount("1231.25"));
    let total = split
        .people
        .iter()
        .fold(Amount::ZERO, |sum, row| sum + row.share);
    assert_eq!(total, split.spent);
}

#[test]
fn money_back_and_write_offs_close_the_event() {
    let mut event = skewers();
    let settle = |id: &str, who: &str, kind: SettlementKind, raw: &str| Settlement {
        id: id.into(),
        kind,
        participant_id: who.into(),
        amount: amount(raw),
        ..Settlement::default()
    };
    event
        .settlements
        .push(settle("s1", "petya", SettlementKind::Transfer, "1735"));
    let split = compute(&event);
    assert_eq!(status(&event, &split), SharedEventStatus::Settling);
    assert_eq!(split.person("petya").unwrap().saldo, amount("0"));
    assert_eq!(split.person(ME).unwrap().saldo, amount("-3805"));

    // a partial return, then the rest written off
    event
        .settlements
        .push(settle("s2", "vasya", SettlementKind::Transfer, "1000"));
    event
        .settlements
        .push(settle("s3", "vasya", SettlementKind::WriteOff, "685"));
    event
        .settlements
        .push(settle("s4", "anya", SettlementKind::Offset, "1060"));
    event
        .settlements
        .push(settle("s5", "olya", SettlementKind::Transfer, "1060"));
    let split = compute(&event);
    assert_eq!(status(&event, &split), SharedEventStatus::Closed);
    assert!(transfer_plan(&event, &split).is_empty());
    // I am out the written-off 685 on top of my own share
    let mine = split.person(ME).unwrap();
    assert_eq!(mine.saldo, amount("0"));
    // a new purchase reopens the settling
    event.items[6]
        .purchases
        .push(purchase("late", "vasya", "1", "100", "rub"));
    let reopened = compute(&event);
    assert_eq!(status(&event, &reopened), SharedEventStatus::Settling);
}

#[test]
fn someone_else_organizes_and_i_owe_them_my_share() {
    let mut event = skewers();
    event.organizer = "anya".into();
    event.items = vec![{
        let mut gift = item("gift", "Подарок", None);
        gift.purchases = vec![purchase("p", "anya", "1", "7400", "rub")];
        gift
    }];
    event.participants = vec![
        person("anya", "Аня"),
        person("petya", "Петя"),
        person("vasya", "Вася"),
    ];
    let split = compute(&event);
    assert_eq!(share_of(&split, ME), amount("1850"));
    assert_eq!(split.person(ME).unwrap().saldo, amount("1850"));
    assert_eq!(split.person("anya").unwrap().saldo, amount("-5550"));
    let plan = transfer_plan(&event, &split);
    assert_eq!(plan.len(), 3);
    assert!(plan.iter().all(|line| line.to == "anya"));
    // I pay my share to Anya
    event.settlements.push(Settlement {
        id: "pay".into(),
        participant_id: ME.into(),
        amount: amount("1850"),
        ..Settlement::default()
    });
    let paid = compute(&event);
    assert!(paid.person(ME).unwrap().saldo.is_zero());
}

#[test]
fn when_i_do_not_take_part_my_share_and_debts_are_nothing() {
    let mut event = skewers();
    event.i_participate = false;
    event.organizer = "petya".into();
    event.items[0].purchases = vec![purchase("p-meat", "petya", "4", "4800", "rub")];
    event.items[1].purchases.clear();
    event.items[6].purchases.clear();
    let split = compute(&event);
    assert!(split.person(ME).is_none());
    assert!(split.purchases.iter().all(|line| line.my_part.is_zero()));
    let shares = split
        .people
        .iter()
        .fold(Amount::ZERO, |sum, row| sum + row.share);
    assert_eq!(shares, amount("4800"));
}

#[test]
fn a_paid_6000_with_my_share_1500_is_1500_of_my_expense() {
    let mut event = skewers();
    event.participants = vec![person("a", "A1"), person("b", "B1"), person("c", "C1")];
    event.rounding_step = amount("0.01");
    event.items = vec![{
        let mut dinner = item("dinner", "Ужин", None);
        dinner.purchases = vec![purchase("p", ME, "1", "6000", "rub")];
        dinner
    }];
    let split = compute(&event);
    assert_eq!(share_of(&split, ME), amount("1500"));
    assert_eq!(split.purchases[0].my_part, amount("1500"));
    assert_eq!(split.person(ME).unwrap().saldo, amount("-4500"));
}

#[test]
fn cancelled_items_and_unbought_plans_stay_out() {
    let mut event = skewers();
    event.items[0].cancelled = true;
    let split = compute(&event);
    assert_eq!(split.spent, amount("2300"));
    assert!(split.items.iter().all(|row| row.item_id != "meat"));
    // price per unit works for plans and purchases alike
    let plan = ItemPlanProbe::per_unit();
    assert_eq!(plan, amount("600"));
    let _ = (EventItem::default(), PriceMode::default());
}

struct ItemPlanProbe;
impl ItemPlanProbe {
    fn per_unit() -> Amount {
        let mut buy = purchase("p", ME, "3", "200", "rub");
        buy.price_mode = PriceMode::PerUnit;
        event_amount("rub", &buy)
    }
}

fn three_people(step: &str, item_title: &str, buyer: &str, total: &str) -> SharedEvent {
    let mut event = skewers();
    event.rounding_step = amount(step);
    event.participants.truncate(2);
    let mut bought = item("x", item_title, None);
    bought.purchases = vec![purchase("px", buyer, "1", total, "rub")];
    event.items = vec![bought];
    event
}

#[test]
fn a_big_step_never_makes_a_share_negative() {
    // bread 150 at step 100 between three: 50 each rounds up to 100 each
    let split = compute(&three_people("100", "Хлеб", "petya", "150"));
    for row in &split.people {
        assert!(!row.share.is_negative(), "{} {}", row.id, row.share);
    }
    let total = split
        .people
        .iter()
        .fold(Amount::ZERO, |sum, row| sum + row.share);
    assert_eq!(total, amount("150"));
    // pizza 1400 between three: 466.67 -> 500 each, the organizer gives back 100
    let split = compute(&three_people("100", "Пицца", "petya", "1400"));
    assert_eq!(share_of(&split, ME), amount("400"));
    assert_eq!(share_of(&split, "petya"), amount("500"));
    assert_eq!(share_of(&split, "vasya"), amount("500"));
}

#[test]
fn rounding_works_on_the_total_of_a_person_not_on_each_item() {
    // two items of 10 between two people at step 10: each item alone would
    // give 5 -> 10 / 0; the totals are 10 and 10
    let mut event = skewers();
    event.rounding_step = amount("10");
    event.participants.truncate(1);
    event.items = vec![
        {
            let mut one = item("a", "A", None);
            one.purchases = vec![purchase("pa", ME, "1", "10", "rub")];
            one
        },
        {
            let mut two = item("b", "B", None);
            two.purchases = vec![purchase("pb", ME, "1", "10", "rub")];
            two
        },
    ];
    let split = compute(&event);
    assert_eq!(share_of(&split, ME), amount("10"));
    assert_eq!(share_of(&split, "petya"), amount("10"));
}

#[test]
fn totals_that_do_not_fit_are_ignored_and_flagged() {
    let mut event = skewers();
    event.adjustments = vec![ShareOverride {
        participant_id: "petya".into(),
        total: amount("10000"),
    }];
    let split = compute(&event);
    assert!(split.overrides_invalid);
    assert_eq!(share_of(&split, "petya"), amount("1735"));
    // everybody fixed but the sum differs from what was bought
    let mut event = skewers();
    event.participants.truncate(1);
    event.adjustments = vec![
        ShareOverride {
            participant_id: "petya".into(),
            total: amount("100"),
        },
        ShareOverride {
            participant_id: ME.into(),
            total: amount("100"),
        },
    ];
    assert!(compute(&event).overrides_invalid);
    // fixed to exactly what was bought: fine
    event.adjustments[0].total = amount("2800");
    event.adjustments[1].total = amount("4300");
    let ok = compute(&event);
    assert!(!ok.overrides_invalid);
    assert_eq!(share_of(&ok, "petya"), amount("2800"));
}

#[test]
fn bought_and_even_closes_without_any_settlement() {
    let mut event = skewers();
    event.participants.truncate(1);
    let mut shared = item("a", "A", None);
    shared.purchases = vec![
        purchase("pa", ME, "1", "500", "rub"),
        purchase("pb", "petya", "1", "500", "rub"),
    ];
    event.items = vec![shared];
    let split = compute(&event);
    assert_eq!(status(&event, &split), SharedEventStatus::Closed);
}

#[test]
fn the_only_sharer_of_an_item_is_found() {
    let mut event = skewers();
    let mut beer = item("b", "Пиво", None);
    beer.split = ItemSplit::Equal {
        among: vec!["petya".into()],
    };
    assert!(only_sharer(&event, &beer, "petya"));
    assert!(!only_sharer(&event, &beer, "vasya"));
    beer.split = ItemSplit::Equal {
        among: vec!["petya".into(), "vasya".into()],
    };
    assert!(!only_sharer(&event, &beer, "petya"));
    event.i_participate = false;
    event.participants.truncate(1);
    assert!(only_sharer(&event, &item("c", "C", None), "petya"));
}
