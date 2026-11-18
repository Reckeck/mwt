//! Settling an event: money back, set-offs, write-offs, passing a rest on to
//! the others. Everything goes through the organizer. Money of mine becomes a
//! debt transfer (`Collect` when I get it back, `Repay` when I give it) linked
//! to the event; the debt itself is never stored, it is the event's balance.
//!
//! Pure part: each `plan_*` checks a request against a document and returns
//! the settlement (and the transfer) to write. The writes are the scenarios in
//! `usecases::household::shared_events`.

use super::split::{self, Split};
use super::{find_event, normalize_moment};
use crate::Amount;
use crate::error::{Error, Result};
use crate::household::contacts::DebtTermsDraft;
use crate::ipc_contract::{SetSharedDueInput, SettleSharedInput, SharedRemainderInput};
use crate::model::VaultDoc;
use crate::model::shared_event::{ME, SettleDirection, Settlement, SettlementKind, SharedEvent};
use crate::model::store::AssetRef;
use crate::model::transaction::{
    AccountRef, DebtEffect, PriceStamp, Transaction, TransferTx, TxMeta,
};

fn tolerance() -> Amount {
    split::settle_tolerance()
}

fn moment(raw: Option<&str>, now: &str) -> Result<String> {
    match raw.map(str::trim).filter(|raw| !raw.is_empty()) {
        Some(raw) => normalize_moment(raw),
        None => Ok(now.to_string()),
    }
}

/// The person, checked: me (when I take part) or a participant, and not the organizer.
fn check_settling_person(event: &SharedEvent, computed: &Split, id: &str) -> Result<()> {
    if id == event.organizer {
        return Err(Error::InvalidInput(
            "the organizer settles with the others, not with themselves".into(),
        ));
    }
    if computed.person(id).is_none() {
        return Err(Error::Missing(format!("participant {id}")));
    }
    Ok(())
}

/// The debt transfer for money of mine moving between me and the other
/// side of the settlement.
pub(crate) fn settlement_transfer(
    event: &SharedEvent,
    settlement: &Settlement,
    store_id: &str,
) -> Result<Transaction> {
    if !event.i_participate {
        return Err(Error::InvalidInput(
            "you do not take part in this event: settle without an operation".into(),
        ));
    }
    let involves_me = event.organizer == ME || settlement.participant_id == ME;
    if !involves_me {
        return Err(Error::InvalidInput(
            "a settlement between other people is a set-off without an operation".into(),
        ));
    }
    let other = if event.organizer == ME {
        settlement.participant_id.as_str()
    } else {
        event.organizer.as_str()
    };
    let contact_id = event
        .participants
        .iter()
        .find(|person| person.id == other)
        .and_then(|person| person.contact_id.clone())
        .ok_or_else(|| {
            Error::InvalidInput(
                "a guest has no contact: settle without an operation, or make them a contact"
                    .into(),
            )
        })?;
    // The money comes to me when the other side pays me.
    let receive = (event.organizer == ME) == (settlement.direction == SettleDirection::ToOrganizer);
    let wallet = AccountRef::Store(store_id.to_string());
    let contact = AccountRef::Contact {
        contact_id,
        account_id: None,
    };
    let (effect, from, to) = if receive {
        (DebtEffect::Collect, contact, wallet)
    } else {
        (DebtEffect::Repay, wallet, contact)
    };
    let mut meta = TxMeta::new(&event.title);
    meta.timestamp = settlement.at.clone();
    let code = event.currency.to_uppercase();
    Ok(Transaction::Transfer(TransferTx {
        meta,
        from,
        to,
        asset: AssetRef::fiat(&code, &code),
        amount: settlement.amount,
        fee: None,
        network: None,
        tx_hash: None,
        debt: Some(effect),
        price: PriceStamp::default(),
    }))
}

/// The rest of what someone owes the organizer, checked, and how much of it to move.
fn open_rest(event: &SharedEvent, participant_id: &str, amount: Option<Amount>) -> Result<Amount> {
    let computed = split::compute(event);
    check_settling_person(event, &computed, participant_id)?;
    let saldo = computed
        .person(participant_id)
        .map_or(Amount::ZERO, |row| row.saldo);
    if !saldo.is_positive() {
        return Err(Error::InvalidInput("nothing is owed here".into()));
    }
    let amount = amount.unwrap_or(saldo);
    if !amount.is_positive() || amount > saldo + tolerance() {
        return Err(Error::InvalidInput(
            "the amount must be positive and at most what is owed".into(),
        ));
    }
    Ok(amount.min(saldo))
}

/// A settlement to record and, when a wallet was named, the operation that
/// goes with it (to be written first; its id then goes onto the settlement).
pub struct SettlePlan {
    pub settlement: Settlement,
    pub transfer: Option<Transaction>,
}

/// Money between a participant and the organizer (or a set-off).
pub fn plan_settle(doc: &VaultDoc, input: &SettleSharedInput, now: &str) -> Result<SettlePlan> {
    let event = find_event(doc, &input.event_id)?;
    let computed = split::compute(event);
    check_settling_person(event, &computed, &input.participant_id)?;
    if !input.amount.is_positive() {
        return Err(Error::InvalidInput("the amount must be positive".into()));
    }
    let saldo = computed
        .person(&input.participant_id)
        .map_or(Amount::ZERO, |row| row.saldo);
    let allowed = match input.direction {
        SettleDirection::ToOrganizer => saldo,
        SettleDirection::FromOrganizer => -saldo,
    };
    if input.amount > allowed + tolerance() {
        return Err(Error::InvalidInput(
            "this is more than is owed in that direction".into(),
        ));
    }
    let settlement = Settlement {
        id: uuid::Uuid::new_v4().to_string(),
        kind: SettlementKind::Offset,
        participant_id: input.participant_id.clone(),
        direction: input.direction,
        amount: input.amount,
        at: moment(input.at.as_deref(), now)?,
        tx_id: None,
        to: Vec::new(),
        organizer: Some(event.organizer.clone()),
    };
    let transfer = input
        .store_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|store_id| settlement_transfer(event, &settlement, store_id))
        .transpose()?;
    Ok(SettlePlan {
        settlement,
        transfer,
    })
}

/// "Write off the rest": I give up what a participant owes me; it becomes my
/// expense in the category of the event.
pub fn plan_write_off(
    doc: &VaultDoc,
    input: &SharedRemainderInput,
    now: &str,
) -> Result<Settlement> {
    let event = find_event(doc, &input.event_id)?;
    if event.organizer != ME || !event.i_participate {
        return Err(Error::InvalidInput(
            "only the organizer can write a debt off, and only when they take part".into(),
        ));
    }
    let amount = open_rest(event, &input.participant_id, input.amount)?;
    Ok(Settlement {
        id: uuid::Uuid::new_v4().to_string(),
        kind: SettlementKind::WriteOff,
        participant_id: input.participant_id.clone(),
        direction: SettleDirection::ToOrganizer,
        amount,
        at: now.to_string(),
        tx_id: None,
        to: Vec::new(),
        organizer: Some(ME.to_string()),
    })
}

/// "Move onto the others": what a participant has not returned is split
/// between the others.
pub fn plan_reassign(
    doc: &VaultDoc,
    input: &SharedRemainderInput,
    now: &str,
) -> Result<Settlement> {
    let event = find_event(doc, &input.event_id)?;
    let amount = open_rest(event, &input.participant_id, input.amount)?;
    let people = split::sharer_pool(event);
    let targets: Vec<String> = if input.to.is_empty() {
        people
            .into_iter()
            .filter(|id| *id != input.participant_id)
            .collect()
    } else {
        if let Some(unknown) = input.to.iter().find(|id| !people.contains(id)) {
            return Err(Error::Missing(format!("participant {unknown}")));
        }
        input
            .to
            .iter()
            .filter(|id| **id != input.participant_id)
            .cloned()
            .collect()
    };
    if targets.is_empty() {
        return Err(Error::InvalidInput("there is nobody to put it on".into()));
    }
    Ok(Settlement {
        id: uuid::Uuid::new_v4().to_string(),
        kind: SettlementKind::Reassign,
        participant_id: input.participant_id.clone(),
        direction: SettleDirection::ToOrganizer,
        amount,
        at: now.to_string(),
        tx_id: None,
        to: targets,
        organizer: Some(event.organizer.clone()),
    })
}

/// A settlement that can be taken back: the id of the operation it made, if any.
pub fn plan_take_back(
    doc: &VaultDoc,
    event_id: &str,
    settlement_id: &str,
) -> Result<Option<String>> {
    let event = find_event(doc, event_id)?;
    event
        .settlements
        .iter()
        .find(|settlement| settlement.id == settlement_id)
        .map(|settlement| settlement.tx_id.clone())
        .ok_or_else(|| Error::Missing(format!("settlement {settlement_id}")))
}

/// The due date of a contact participant's debt (the existing debt terms,
/// keyed by contact and currency); an empty date clears it.
pub fn plan_due(doc: &VaultDoc, input: SetSharedDueInput) -> Result<DebtTermsDraft> {
    let event = find_event(doc, &input.event_id)?;
    let contact_id = event
        .participants
        .iter()
        .find(|person| person.id == input.participant_id)
        .and_then(|person| person.contact_id.clone())
        .ok_or_else(|| Error::InvalidInput("a term needs a participant who is a contact".into()))?;
    let asset = event.currency.to_uppercase();
    let note = doc
        .debt_terms
        .iter()
        .find(|terms| terms.contact_id == contact_id && terms.asset == asset)
        .map(|terms| terms.note.clone())
        .unwrap_or_default();
    Ok(DebtTermsDraft {
        contact_id,
        asset,
        due_at: input.due_at,
        note,
    })
}
