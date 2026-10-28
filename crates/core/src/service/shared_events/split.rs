//! The split of an event: who owes what, worked out from the list alone.
//!
//! Everything is in the event currency. A purchase is converted once, at the
//! rate stored on it, and rounded to cents; the shares of an item are rounded
//! to the event's step and the leftover goes to the item's buyer, so the shares
//! always add up to what was bought, to the cent.

use std::collections::BTreeMap;

use rust_decimal::{Decimal, RoundingStrategy};

use crate::Amount;
use crate::model::shared_event::{
    ItemSplit, ME, Purchase, SettleDirection, SettlementKind, SharedEvent, SharedEventStatus,
};

/// One person's numbers in an event.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonRow {
    /// [`ME`] or a participant id.
    pub id: String,
    /// What the person's part of the purchases comes to.
    pub share: Amount,
    /// What the person paid out of their own pocket.
    pub paid: Amount,
    /// Net money handed to the organizer so far (negative: received from them).
    pub moved: Amount,
    /// The part of their debt the organizer wrote off.
    pub written_off: Amount,
    /// What the person still owes the group (negative: is owed). For everyone
    /// but the organizer this is the amount settled through the organizer.
    pub saldo: Amount,
}

/// One purchase, converted to the event currency.
#[derive(Debug, Clone, PartialEq)]
pub struct PurchaseLine {
    pub item_id: String,
    pub purchase_id: String,
    pub buyer: String,
    pub at: String,
    pub event_amount: Amount,
    /// The part of it that is my expense (analytics); zero when I do not take part.
    pub my_part: Amount,
}

/// One item's numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemRow {
    pub item_id: String,
    /// Bought, in the event currency.
    pub cost: Amount,
    /// Bought, in units.
    pub qty: Amount,
    pub plan_total: Option<Amount>,
    pub plan_qty: Option<Amount>,
    /// Each sharer's part of this item (before hand-set totals).
    pub shares: BTreeMap<String, Amount>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Split {
    pub people: Vec<PersonRow>,
    pub items: Vec<ItemRow>,
    pub purchases: Vec<PurchaseLine>,
    /// Everything bought.
    pub spent: Amount,
    /// Everything planned for the items not cancelled.
    pub planned: Amount,
}

impl Split {
    pub fn person(&self, id: &str) -> Option<&PersonRow> {
        self.people.iter().find(|row| row.id == id)
    }

    /// Whether every balance is settled (to the cent).
    pub fn all_settled(&self) -> bool {
        self.people.iter().all(|row| is_zero(row.saldo))
    }
}

pub(crate) fn one() -> Amount {
    Amount::new(Decimal::ONE)
}

fn is_zero(value: Amount) -> bool {
    value.abs().0 < Decimal::new(5, 3)
}

/// Round half away from zero to `places` decimals.
pub(crate) fn round_places(value: Amount, places: u32) -> Amount {
    Amount::new(
        value
            .0
            .round_dp_with_strategy(places, RoundingStrategy::MidpointAwayFromZero),
    )
}

fn round_to_step(value: Amount, step: Amount) -> Amount {
    if !step.is_positive() {
        return round_places(value, 2);
    }
    let steps =
        (value.0 / step.0).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    Amount::new(steps * step.0)
}

/// What a purchase is worth in the event currency.
pub fn event_amount(event_currency: &str, purchase: &Purchase) -> Amount {
    let rate = if purchase.currency.eq_ignore_ascii_case(event_currency) {
        one()
    } else {
        purchase.rate_to_event
    };
    round_places(purchase.total() * rate, 2)
}

/// Everyone who takes part in the split, the owner first.
pub fn people_ids(event: &SharedEvent) -> Vec<String> {
    let mut ids = Vec::new();
    if event.i_participate || event.organizer == ME {
        ids.push(ME.to_string());
    }
    ids.extend(event.participants.iter().map(|person| person.id.clone()));
    ids
}

/// Who can share an item: everybody, but the owner only when they take part.
fn sharer_pool(event: &SharedEvent) -> Vec<String> {
    people_ids(event)
        .into_iter()
        .filter(|id| id != ME || event.i_participate)
        .collect()
}

/// Weights of the people sharing an item.
fn weights(event: &SharedEvent, split: &ItemSplit, pool: &[String]) -> Vec<(String, Amount)> {
    let everyone =
        || -> Vec<(String, Amount)> { pool.iter().map(|id| (id.clone(), one())).collect() };
    match split {
        ItemSplit::Equal { among } if among.is_empty() => everyone(),
        ItemSplit::Equal { among } => {
            let picked: Vec<(String, Amount)> = pool
                .iter()
                .filter(|id| among.contains(id))
                .map(|id| (id.clone(), one()))
                .collect();
            if picked.is_empty() {
                everyone()
            } else {
                picked
            }
        }
        ItemSplit::ByUnits { units } => {
            let picked: Vec<(String, Amount)> = pool
                .iter()
                .filter_map(|id| {
                    units
                        .get(id)
                        .filter(|qty| qty.is_positive())
                        .map(|qty| (id.clone(), *qty))
                })
                .collect();
            if picked.is_empty() {
                everyone()
            } else {
                picked
            }
        }
    }
    .into_iter()
    .filter(|(id, _)| {
        // A removed participant's weight is gone with them.
        id == ME || event.participants.iter().any(|person| &person.id == id)
    })
    .collect()
}

/// Spread `cost` over `weights`, each part rounded to `step`; the leftover goes
/// to `taker` when they share, else to the first sharer.
fn spread(
    cost: Amount,
    weights: &[(String, Amount)],
    step: Amount,
    taker: &str,
) -> BTreeMap<String, Amount> {
    let mut parts = BTreeMap::new();
    let total_weight = weights
        .iter()
        .fold(Amount::ZERO, |sum, (_, weight)| sum + *weight);
    if weights.is_empty() || !total_weight.is_positive() {
        return parts;
    }
    let mut given = Amount::ZERO;
    for (id, weight) in weights {
        let part = round_to_step(cost * *weight / total_weight, step);
        given += part;
        parts.insert(id.clone(), part);
    }
    let receiver = if parts.contains_key(taker) {
        taker.to_string()
    } else {
        weights[0].0.clone()
    };
    if let Some(part) = parts.get_mut(&receiver) {
        *part += cost - given;
    }
    parts
}

pub fn compute(event: &SharedEvent) -> Split {
    let pool = sharer_pool(event);
    let step = event.rounding_step;
    let mut items = Vec::new();
    let mut purchases: Vec<PurchaseLine> = Vec::new();
    let mut shares: BTreeMap<String, Amount> = BTreeMap::new();
    let mut paid: BTreeMap<String, Amount> = BTreeMap::new();
    let mut spent = Amount::ZERO;
    let mut planned = Amount::ZERO;

    for item in event.items.iter().filter(|item| !item.cancelled) {
        let lines: Vec<(&Purchase, Amount)> = item
            .purchases
            .iter()
            .map(|purchase| (purchase, event_amount(&event.currency, purchase)))
            .collect();
        let cost = lines
            .iter()
            .fold(Amount::ZERO, |sum, (_, amount)| sum + *amount);
        let qty = item
            .purchases
            .iter()
            .fold(Amount::ZERO, |sum, purchase| sum + purchase.qty);
        let plan_total = item.plan.as_ref().map(|plan| round_places(plan.total(), 2));
        if let Some(total) = plan_total {
            planned += total;
        }
        // The biggest purchase's buyer absorbs the rounding leftover.
        let mut buyer = String::new();
        let mut biggest = Amount::ZERO;
        for (purchase, amount) in &lines {
            if buyer.is_empty() || *amount > biggest {
                buyer = purchase.buyer.clone();
                biggest = *amount;
            }
            *paid.entry(purchase.buyer.clone()).or_default() += *amount;
            purchases.push(PurchaseLine {
                item_id: item.id.clone(),
                purchase_id: purchase.id.clone(),
                buyer: purchase.buyer.clone(),
                at: purchase.at.clone(),
                event_amount: *amount,
                my_part: Amount::ZERO,
            });
        }
        spent += cost;
        let item_shares = spread(cost, &weights(event, &item.split, &pool), step, &buyer);
        for (id, part) in &item_shares {
            *shares.entry(id.clone()).or_default() += *part;
        }
        items.push(ItemRow {
            item_id: item.id.clone(),
            cost,
            qty,
            plan_total,
            plan_qty: item.plan.as_ref().map(|plan| plan.qty),
            shares: item_shares,
        });
    }

    apply_overrides(event, &pool, &mut shares, &paid, spent);
    apply_reassignments(event, &mut shares);

    let mut moved: BTreeMap<String, Amount> = BTreeMap::new();
    let mut written_off: BTreeMap<String, Amount> = BTreeMap::new();
    for settlement in &event.settlements {
        match settlement.kind {
            SettlementKind::Transfer | SettlementKind::Offset => {
                let signed = match settlement.direction {
                    SettleDirection::ToOrganizer => settlement.amount,
                    SettleDirection::FromOrganizer => -settlement.amount,
                };
                *moved.entry(settlement.participant_id.clone()).or_default() += signed;
            }
            SettlementKind::WriteOff => {
                *written_off
                    .entry(settlement.participant_id.clone())
                    .or_default() += settlement.amount;
            }
            SettlementKind::Reassign => {}
        }
    }

    let mut people: Vec<PersonRow> = people_ids(event)
        .into_iter()
        .map(|id| {
            let share = shares.get(&id).copied().unwrap_or_default();
            let paid_by = paid.get(&id).copied().unwrap_or_default();
            let moved_by = moved.get(&id).copied().unwrap_or_default();
            let off = written_off.get(&id).copied().unwrap_or_default();
            PersonRow {
                saldo: share - paid_by - moved_by - off,
                id,
                share,
                paid: paid_by,
                moved: moved_by,
                written_off: off,
            }
        })
        .collect();
    // Everything goes through the organizer, so theirs is the opposite of the rest.
    let others = people
        .iter()
        .filter(|row| row.id != event.organizer)
        .fold(Amount::ZERO, |sum, row| sum + row.saldo);
    if let Some(row) = people.iter_mut().find(|row| row.id == event.organizer) {
        row.saldo = -others;
    }

    let my_share = people
        .iter()
        .find(|row| row.id == ME)
        .map_or(Amount::ZERO, |row| row.share);
    distribute_my_part(&mut purchases, &items, my_share);

    Split {
        people,
        items,
        purchases,
        spent,
        planned,
    }
}

/// Hand-set totals: those people get exactly that, the others share the rest
/// in proportion to what they would have owed.
fn apply_overrides(
    event: &SharedEvent,
    pool: &[String],
    shares: &mut BTreeMap<String, Amount>,
    paid: &BTreeMap<String, Amount>,
    spent: Amount,
) {
    let fixed: Vec<(&String, Amount)> = event
        .adjustments
        .iter()
        .filter(|fix| pool.contains(&fix.participant_id))
        .map(|fix| (&fix.participant_id, fix.total))
        .collect();
    if fixed.is_empty() {
        return;
    }
    let fixed_sum = fixed
        .iter()
        .fold(Amount::ZERO, |sum, (_, total)| sum + *total);
    let rest = spent - fixed_sum;
    let others: Vec<&String> = pool
        .iter()
        .filter(|id| !fixed.iter().any(|(fixed_id, _)| fixed_id == id))
        .collect();
    if others.is_empty() {
        for (id, total) in &fixed {
            shares.insert((*id).clone(), *total);
        }
        return;
    }
    let weights: Vec<(String, Amount)> = others
        .iter()
        .map(|id| {
            let base = shares.get(*id).copied().unwrap_or_default();
            ((*id).clone(), base)
        })
        .collect();
    let base_sum = weights
        .iter()
        .fold(Amount::ZERO, |sum, (_, weight)| sum + *weight);
    let weights = if base_sum.is_positive() {
        weights
    } else {
        // Nobody else had a part: the rest is split evenly.
        weights
            .into_iter()
            .map(|(id, _)| (id, one()))
            .collect::<Vec<_>>()
    };
    // The leftover of the rounding goes to whoever paid the most among the others.
    let taker = others
        .iter()
        .max_by_key(|id| paid.get(**id).copied().unwrap_or_default())
        .map(|id| (*id).clone())
        .unwrap_or_default();
    let spread_rest = spread(rest, &weights, event.rounding_step, &taker);
    for (id, total) in &fixed {
        shares.insert((*id).clone(), *total);
    }
    for id in others {
        shares.insert(id.clone(), spread_rest.get(id).copied().unwrap_or_default());
    }
}

/// A rest put on the others: the debtor's share shrinks, theirs grow.
fn apply_reassignments(event: &SharedEvent, shares: &mut BTreeMap<String, Amount>) {
    for settlement in event
        .settlements
        .iter()
        .filter(|settlement| settlement.kind == SettlementKind::Reassign)
    {
        let targets: Vec<&String> = settlement
            .to
            .iter()
            .filter(|id| **id != settlement.participant_id)
            .collect();
        if targets.is_empty() {
            continue;
        }
        *shares.entry(settlement.participant_id.clone()).or_default() -= settlement.amount;
        let count = Amount::new(Decimal::from(targets.len() as u64));
        let each = round_places(settlement.amount / count, 2);
        let mut given = Amount::ZERO;
        for id in &targets {
            *shares.entry((*id).clone()).or_default() += each;
            given += each;
        }
        if let Some(first) = targets.first() {
            *shares.entry((*first).clone()).or_default() += settlement.amount - given;
        }
    }
}

/// Spread my total share over the purchases, item by item where my share of the
/// item is known: that is the part of each purchase that is my expense.
fn distribute_my_part(purchases: &mut [PurchaseLine], items: &[ItemRow], my_share: Amount) {
    if purchases.is_empty() || my_share.is_zero() {
        return;
    }
    let mut weight_of: Vec<Amount> = Vec::with_capacity(purchases.len());
    for line in purchases.iter() {
        let item = items.iter().find(|row| row.item_id == line.item_id);
        let weight = match item {
            Some(row) if row.cost.is_positive() => {
                row.shares.get(ME).copied().unwrap_or_default() * line.event_amount / row.cost
            }
            _ => Amount::ZERO,
        };
        weight_of.push(weight);
    }
    let mut total_weight = weight_of
        .iter()
        .fold(Amount::ZERO, |sum, weight| sum + *weight);
    if !total_weight.is_positive() {
        weight_of = purchases.iter().map(|line| line.event_amount).collect();
        total_weight = weight_of
            .iter()
            .fold(Amount::ZERO, |sum, weight| sum + *weight);
        if !total_weight.is_positive() {
            return;
        }
    }
    let last = purchases.len() - 1;
    let mut given = Amount::ZERO;
    for (index, line) in purchases.iter_mut().enumerate() {
        line.my_part = if index == last {
            my_share - given
        } else {
            round_places(my_share * weight_of[index] / total_weight, 2)
        };
        given += line.my_part;
    }
}

/// Where the event stands.
pub fn status(event: &SharedEvent, split: &Split) -> SharedEventStatus {
    let bought = event
        .items
        .iter()
        .any(|item| !item.cancelled && !item.purchases.is_empty());
    if !bought {
        SharedEventStatus::Planning
    } else if event.settlements.is_empty() {
        SharedEventStatus::InProgress
    } else if split.all_settled() {
        SharedEventStatus::Closed
    } else {
        SharedEventStatus::Settling
    }
}

/// One payment of the plan «who pays whom» (through the organizer).
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedTransfer {
    pub from: String,
    pub to: String,
    pub amount: Amount,
}

pub fn transfer_plan(event: &SharedEvent, split: &Split) -> Vec<PlannedTransfer> {
    split
        .people
        .iter()
        .filter(|row| row.id != event.organizer && !is_zero(row.saldo))
        .map(|row| {
            if row.saldo.is_positive() {
                PlannedTransfer {
                    from: row.id.clone(),
                    to: event.organizer.clone(),
                    amount: row.saldo,
                }
            } else {
                PlannedTransfer {
                    from: event.organizer.clone(),
                    to: row.id.clone(),
                    amount: -row.saldo,
                }
            }
        })
        .collect()
}

#[cfg(test)]
pub(super) mod fixtures {
    use super::*;
    use crate::model::shared_event::{EventItem, ItemPlan, Participant, PriceMode};

    pub fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    pub fn person(id: &str, name: &str) -> Participant {
        Participant {
            id: id.into(),
            name: name.into(),
            ..Participant::default()
        }
    }

    pub fn purchase(id: &str, buyer: &str, qty: &str, total: &str, currency: &str) -> Purchase {
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

    pub fn item(id: &str, title: &str, plan: Option<(&str, &str)>) -> EventItem {
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

    /// The «Шашлыки» board: me, Petya, Vasya, Anya and the guest Olya, in rubles.
    pub fn skewers() -> SharedEvent {
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

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use crate::model::shared_event::{EventItem, PriceMode, Settlement, ShareOverride};

    fn share_of(split: &Split, id: &str) -> Amount {
        split.person(id).unwrap().share
    }

    #[test]
    fn the_skewers_board_adds_up_to_the_cent() {
        let event = skewers();
        let split = compute(&event);
        assert_eq!(split.planned, amount("9400"));
        assert_eq!(split.spent, amount("7100"));
        assert_eq!(share_of(&split, ME), amount("1060"));
        assert_eq!(share_of(&split, "petya"), amount("1735"));
        assert_eq!(share_of(&split, "vasya"), amount("2185"));
        assert_eq!(share_of(&split, "anya"), amount("1060"));
        assert_eq!(share_of(&split, "olya"), amount("1060"));
        let shares = split
            .people
            .iter()
            .fold(Amount::ZERO, |sum, row| sum + row.share);
        assert_eq!(shares, split.spent);
        // me: paid 6600, share 1060 -> they owe me 5540
        assert_eq!(split.person(ME).unwrap().paid, amount("6600"));
        assert_eq!(split.person(ME).unwrap().saldo, amount("-5540"));
        assert_eq!(split.person("petya").unwrap().saldo, amount("1735"));
        assert_eq!(split.person("vasya").unwrap().saldo, amount("1685"));
        assert_eq!(split.person("anya").unwrap().saldo, amount("1060"));
        let saldo_sum = split
            .people
            .iter()
            .fold(Amount::ZERO, |sum, row| sum + row.saldo);
        assert!(saldo_sum.is_zero());
        assert_eq!(status(&event, &split), SharedEventStatus::InProgress);
        // beer: 8 bought against 6 planned, 600 over
        let beer = split
            .items
            .iter()
            .find(|row| row.item_id == "beer")
            .unwrap();
        assert_eq!(beer.cost - beer.plan_total.unwrap(), amount("600"));
        assert_eq!(beer.qty, amount("8"));
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
        // 100 / 3 = 33.33 -> 33 each at step 1; the leftover 1 goes to the buyer
        let split = compute(&event);
        assert_eq!(share_of(&split, ME), amount("33"));
        assert_eq!(share_of(&split, "petya"), amount("33"));
        assert_eq!(share_of(&split, "vasya"), amount("34"));
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
}
