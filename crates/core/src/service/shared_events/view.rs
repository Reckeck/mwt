//! What the UI shows of an event: the stored list plus everything worked out
//! from it (shares, balances, plan of payments, status, my position).

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::shared_event::{ME, Participant, SharedEvent, SharedEventStatus, SharedLink};
use crate::model::transaction::Transaction;
use crate::service::UnlockedVault;

use super::split::{self, Split};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum PersonKind {
    #[default]
    Me,
    Contact,
    Guest,
}

/// One row of the people table.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PersonView {
    /// `me` or a participant id.
    pub id: String,
    /// Empty for me (the UI says «Я»).
    pub name: String,
    pub kind: PersonKind,
    pub contact_id: Option<String>,
    /// A guest with phone, messenger, requisites or a note.
    pub has_details: bool,
    pub is_organizer: bool,
    /// What the person's part comes to.
    pub share: Amount,
    /// What they paid themselves.
    pub paid: Amount,
    /// Net money handed to the organizer so far (negative: got from them).
    pub moved: Amount,
    pub written_off: Amount,
    /// Still to pay (positive) or to receive (negative), through the organizer.
    pub saldo: Amount,
    /// The term set for this person's debt, if any (contacts only).
    pub due_at: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ItemShareView {
    pub participant_id: String,
    pub amount: Amount,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ItemView {
    pub item_id: String,
    /// Bought, in the event currency.
    pub cost: Amount,
    /// Bought, in units.
    pub qty: Amount,
    pub plan_total: Option<Amount>,
    pub plan_qty: Option<Amount>,
    /// `cost - plan_total`; zero without a plan or a purchase.
    pub diff_money: Amount,
    /// `qty - plan_qty`; zero without a plan or a purchase.
    pub diff_qty: Amount,
    pub bought: bool,
    pub shares: Vec<ItemShareView>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PurchaseView {
    pub item_id: String,
    pub purchase_id: String,
    /// In the event currency.
    pub event_amount: Amount,
    /// The part of it that is my expense.
    pub my_part: Amount,
    /// The linked payment is gone from the vault.
    pub payment_missing: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SettlementView {
    pub settlement_id: String,
    pub payment_missing: bool,
}

/// One payment of the plan «who pays whom».
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PlannedPayment {
    pub from: String,
    pub to: String,
    pub amount: Amount,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct EventTotals {
    pub planned: Amount,
    pub spent: Amount,
    /// `spent - planned`.
    pub diff: Amount,
}

/// «Моя позиция».
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct MyPosition {
    pub paid: Amount,
    pub share: Amount,
    /// Still owed to me.
    pub owed_to_me: Amount,
    /// Money already returned to me (net of what I gave back).
    pub returned: Amount,
    /// Still owed by me (to the organizer, or to people who paid for me).
    pub i_owe: Amount,
    pub written_off: Amount,
    /// Out of pocket beyond my share: `owed_to_me - i_owe + written_off`.
    pub over_share: Amount,
    /// The organizer I owe my share to, when it is not me.
    pub pay_to: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SharedEventView {
    pub event: SharedEvent,
    pub status: SharedEventStatus,
    pub totals: EventTotals,
    pub people: Vec<PersonView>,
    pub items: Vec<ItemView>,
    pub purchases: Vec<PurchaseView>,
    pub settlements: Vec<SettlementView>,
    pub plan: Vec<PlannedPayment>,
    /// Absent when I do not take part.
    pub mine: Option<MyPosition>,
}

/// A row of the list of events.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SharedEventSummary {
    pub id: String,
    pub title: String,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
    pub currency: String,
    pub status: SharedEventStatus,
    pub archived: bool,
    pub i_participate: bool,
    pub people_count: u32,
    pub organizer_name: String,
    pub organizer_is_me: bool,
    pub planned: Amount,
    pub spent: Amount,
    pub owed_to_me: Amount,
    pub i_owe: Amount,
}

/// What one event does to one contact's debt with me.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ContactEventBalance {
    pub event_id: String,
    pub title: String,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
    pub currency: String,
    pub status: SharedEventStatus,
    /// Positive: the contact owes me; negative: I owe them.
    pub amount: Amount,
    pub due_at: Option<String>,
}

fn name_of(event: &SharedEvent, id: &str) -> String {
    if id == ME {
        return String::new();
    }
    event
        .participants
        .iter()
        .find(|person| person.id == id)
        .map(|person| person.name.clone())
        .unwrap_or_default()
}

impl UnlockedVault {
    pub(super) fn find_shared_event(&self, id: &str) -> Result<&SharedEvent> {
        self.doc
            .shared_events
            .iter()
            .find(|event| event.id == id)
            .ok_or_else(|| Error::Missing(format!("shared event {id}")))
    }

    pub(super) fn find_shared_event_mut(&mut self, id: &str) -> Result<&mut SharedEvent> {
        self.doc
            .shared_events
            .iter_mut()
            .find(|event| event.id == id)
            .ok_or_else(|| Error::Missing(format!("shared event {id}")))
    }

    fn tx_exists(&self, tx_id: &str) -> bool {
        self.doc
            .transactions
            .iter()
            .any(|tx: &Transaction| tx.meta().id == tx_id)
    }

    fn contact_term(&self, event: &SharedEvent, participant: &Participant) -> Option<String> {
        let contact_id = participant.contact_id.as_deref()?;
        let asset = event.currency.to_uppercase();
        self.doc
            .debt_terms
            .iter()
            .find(|terms| terms.contact_id == contact_id && terms.asset == asset)
            .and_then(|terms| terms.due_at.clone())
    }

    /// The event with everything worked out.
    pub fn shared_event_view(&self, id: &str) -> Result<SharedEventView> {
        let event = self.find_shared_event(id)?;
        let computed = split::compute(event);
        Ok(self.build_view(event, &computed))
    }

    fn build_view(&self, event: &SharedEvent, computed: &Split) -> SharedEventView {
        let people = computed
            .people
            .iter()
            .map(|row| {
                let participant = event.participants.iter().find(|person| person.id == row.id);
                let kind = match participant {
                    None => PersonKind::Me,
                    Some(person) if person.contact_id.is_some() => PersonKind::Contact,
                    Some(_) => PersonKind::Guest,
                };
                PersonView {
                    id: row.id.clone(),
                    name: name_of(event, &row.id),
                    kind,
                    contact_id: participant.and_then(|person| person.contact_id.clone()),
                    has_details: participant
                        .and_then(|person| person.details.as_ref())
                        .is_some_and(|details| !details.is_empty()),
                    is_organizer: row.id == event.organizer,
                    share: row.share,
                    paid: row.paid,
                    moved: row.moved,
                    written_off: row.written_off,
                    saldo: row.saldo,
                    due_at: participant.and_then(|person| self.contact_term(event, person)),
                }
            })
            .collect();
        let items = computed
            .items
            .iter()
            .map(|row| {
                let bought = row.cost.is_positive() || row.qty.is_positive();
                ItemView {
                    item_id: row.item_id.clone(),
                    cost: row.cost,
                    qty: row.qty,
                    plan_total: row.plan_total,
                    plan_qty: row.plan_qty,
                    diff_money: match row.plan_total {
                        Some(plan) if bought => row.cost - plan,
                        _ => Amount::ZERO,
                    },
                    diff_qty: match row.plan_qty {
                        Some(plan) if bought => row.qty - plan,
                        _ => Amount::ZERO,
                    },
                    bought,
                    shares: row
                        .shares
                        .iter()
                        .map(|(participant_id, amount)| ItemShareView {
                            participant_id: participant_id.clone(),
                            amount: *amount,
                        })
                        .collect(),
                }
            })
            .collect();
        let purchase_tx = |purchase_id: &str| {
            event
                .items
                .iter()
                .flat_map(|item| &item.purchases)
                .find(|purchase| purchase.id == purchase_id)
                .and_then(|purchase| purchase.tx_id.clone())
        };
        let purchases = computed
            .purchases
            .iter()
            .map(|line| PurchaseView {
                item_id: line.item_id.clone(),
                purchase_id: line.purchase_id.clone(),
                event_amount: line.event_amount,
                my_part: line.my_part,
                payment_missing: purchase_tx(&line.purchase_id)
                    .is_some_and(|tx_id| !self.tx_exists(&tx_id)),
            })
            .collect();
        let settlements = event
            .settlements
            .iter()
            .map(|settlement| SettlementView {
                settlement_id: settlement.id.clone(),
                payment_missing: settlement
                    .tx_id
                    .as_deref()
                    .is_some_and(|tx_id| !self.tx_exists(tx_id)),
            })
            .collect();
        let plan = split::transfer_plan(event, computed)
            .into_iter()
            .map(|line| PlannedPayment {
                from: line.from,
                to: line.to,
                amount: line.amount,
            })
            .collect();
        SharedEventView {
            event: event.clone(),
            status: split::status(event, computed),
            totals: EventTotals {
                planned: computed.planned,
                spent: computed.spent,
                diff: computed.spent - computed.planned,
            },
            people,
            items,
            purchases,
            settlements,
            plan,
            mine: my_position(event, computed),
        }
    }

    /// The events, newest first by their date (`starts_on`, else the day they
    /// were made); archived ones only when asked for.
    pub fn list_shared_events(&self, include_archived: bool) -> Vec<SharedEventSummary> {
        let mut events: Vec<&SharedEvent> = self
            .doc
            .shared_events
            .iter()
            .filter(|event| include_archived || !event.archived)
            .collect();
        events.sort_by(|left, right| {
            sort_key(right)
                .cmp(&sort_key(left))
                .then_with(|| right.created_at.cmp(&left.created_at))
        });
        events
            .into_iter()
            .map(|event| {
                let computed = split::compute(event);
                let mine = my_position(event, &computed);
                SharedEventSummary {
                    id: event.id.clone(),
                    title: event.title.clone(),
                    starts_on: event.starts_on.clone(),
                    ends_on: event.ends_on.clone(),
                    currency: event.currency.clone(),
                    status: split::status(event, &computed),
                    archived: event.archived,
                    i_participate: event.i_participate,
                    people_count: computed.people.len() as u32,
                    organizer_name: name_of(event, &event.organizer),
                    organizer_is_me: event.organizer == ME,
                    planned: computed.planned,
                    spent: computed.spent,
                    owed_to_me: mine.as_ref().map_or(Amount::ZERO, |mine| mine.owed_to_me),
                    i_owe: mine.as_ref().map_or(Amount::ZERO, |mine| mine.i_owe),
                }
            })
            .collect()
    }

    /// The events that left a debt with (or credit from) a contact.
    pub fn contact_shared_events(&self, contact_id: &str) -> Vec<ContactEventBalance> {
        let mut out = Vec::new();
        for event in &self.doc.shared_events {
            let computed = split::compute(event);
            for (debt_contact, amount) in event_debts(event, &computed) {
                if debt_contact != contact_id || split::round_places(amount, 2).is_zero() {
                    continue;
                }
                let participant = event
                    .participants
                    .iter()
                    .find(|person| person.contact_id.as_deref() == Some(contact_id));
                out.push(ContactEventBalance {
                    event_id: event.id.clone(),
                    title: event.title.clone(),
                    starts_on: event.starts_on.clone(),
                    ends_on: event.ends_on.clone(),
                    currency: event.currency.clone(),
                    status: split::status(event, &computed),
                    amount,
                    due_at: participant.and_then(|person| self.contact_term(event, person)),
                });
            }
        }
        out
    }

    /// Every event debt with a contact, for the contact balances: positive
    /// means the contact owes me.
    pub(crate) fn shared_debts(&self) -> Vec<SharedDebt> {
        let mut out = Vec::new();
        for event in &self.doc.shared_events {
            let computed = split::compute(event);
            for (contact_id, amount) in event_debts(event, &computed) {
                if split::round_places(amount, 2).is_zero() {
                    continue;
                }
                out.push(SharedDebt {
                    contact_id,
                    currency: event.currency.clone(),
                    amount,
                });
            }
        }
        out
    }
}

/// A payment or debt transfer of the vault that an event made or adopted.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SharedTxLink {
    pub tx_id: String,
    pub event_title: String,
    pub link: SharedLink,
}

impl UnlockedVault {
    /// Every operation tied to an event, found through the events (the
    /// operations themselves carry no mark, so older vault files still read).
    pub fn shared_tx_links(&self) -> Vec<SharedTxLink> {
        let mut out = Vec::new();
        for event in &self.doc.shared_events {
            for purchase in event.items.iter().flat_map(|item| &item.purchases) {
                if let Some(tx_id) = &purchase.tx_id {
                    out.push(SharedTxLink {
                        tx_id: tx_id.clone(),
                        event_title: event.title.clone(),
                        link: SharedLink {
                            event_id: event.id.clone(),
                            purchase_id: Some(purchase.id.clone()),
                            settlement_id: None,
                        },
                    });
                }
            }
            for settlement in &event.settlements {
                if let Some(tx_id) = &settlement.tx_id {
                    out.push(SharedTxLink {
                        tx_id: tx_id.clone(),
                        event_title: event.title.clone(),
                        link: SharedLink {
                            event_id: event.id.clone(),
                            purchase_id: None,
                            settlement_id: Some(settlement.id.clone()),
                        },
                    });
                }
            }
        }
        out
    }
}

/// One event's debt with one contact.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SharedDebt {
    pub contact_id: String,
    pub currency: String,
    pub amount: Amount,
}

fn sort_key(event: &SharedEvent) -> String {
    event
        .starts_on
        .clone()
        .unwrap_or_else(|| event.created_at.chars().take(10).collect())
}

/// Debts with contacts that involve me: positive means the contact owes me.
/// Nothing when I do not take part.
fn event_debts(event: &SharedEvent, computed: &Split) -> Vec<(String, Amount)> {
    if !event.i_participate {
        return Vec::new();
    }
    let mut out = Vec::new();
    if event.organizer == ME {
        for row in &computed.people {
            let contact = event
                .participants
                .iter()
                .find(|person| person.id == row.id)
                .and_then(|person| person.contact_id.clone());
            if let Some(contact_id) = contact {
                out.push((contact_id, row.saldo));
            }
        }
    } else if let Some(contact_id) = event
        .participants
        .iter()
        .find(|person| person.id == event.organizer)
        .and_then(|person| person.contact_id.clone())
        && let Some(mine) = computed.person(ME)
    {
        out.push((contact_id, -mine.saldo));
    }
    out
}

fn my_position(event: &SharedEvent, computed: &Split) -> Option<MyPosition> {
    if !event.i_participate {
        return None;
    }
    let mine = computed.person(ME)?;
    let mut position = MyPosition {
        paid: mine.paid,
        share: mine.share,
        ..MyPosition::default()
    };
    if event.organizer == ME {
        for row in computed.people.iter().filter(|row| row.id != ME) {
            if row.saldo.is_positive() {
                position.owed_to_me += row.saldo;
            } else {
                position.i_owe += -row.saldo;
            }
            position.returned += row.moved;
            position.written_off += row.written_off;
        }
    } else {
        if mine.saldo.is_positive() {
            position.i_owe = mine.saldo;
            position.pay_to = Some(event.organizer.clone());
        } else {
            position.owed_to_me = -mine.saldo;
        }
        position.returned = -mine.moved;
    }
    position.over_share = position.owed_to_me - position.i_owe + position.written_off;
    Some(position)
}
