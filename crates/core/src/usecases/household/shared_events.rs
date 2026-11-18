//! Scenarios of the shared events area (the event, its people and items, and
//! settling): each decides in `household::shared_events`, then ends in one
//! `vault.transact`. A settlement that makes a real operation writes the
//! operation first through the transactions area, then records the settlement.

use crate::error::{Error, Result};
use crate::household::shared_events::settle::{
    plan_due, plan_reassign, plan_settle, plan_take_back, plan_write_off,
};
use crate::household::shared_events::{
    check_archivable, find_event, plan_add_item, plan_add_participant, plan_create_event,
    plan_delete_item, plan_guest_contact, plan_item_split, plan_remove_participant,
    plan_share_override, plan_update_event, plan_update_item, plan_update_participant,
};
use crate::ipc_contract::{
    SetShareOverrideInput, SetSharedDueInput, SetSharedItemSplitInput, SettleSharedInput,
    SharedEventDraft, SharedItemDraft, SharedParticipantDraft, SharedRemainderInput,
};
use crate::kernel::tx::Tx;
use crate::model::contact::{Contact, DebtTerms};
use crate::model::shared_event::{Settlement, SharedEvent};
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::service::{SharedEventView, UnlockedVault};

/// Put the event in place of its older self. An archived event that was
/// edited is open again.
fn store_event(tx: &mut Tx<'_>, mut event: SharedEvent) -> Result<()> {
    event.archived = false;
    let slot = tx
        .doc_mut()
        .shared_events
        .iter_mut()
        .find(|known| known.id == event.id)
        .ok_or_else(|| Error::Missing(format!("shared event {}", event.id)))?;
    *slot = event;
    Ok(())
}

/// Write the changed event and read it back as the view.
fn commit_event(vault: &mut UnlockedVault, event: SharedEvent) -> Result<SharedEventView> {
    let id = event.id.clone();
    vault.transact(|tx| store_event(tx, event))?;
    vault.shared_event_view(&id)
}

/// Save after a change to the event: an archived event that was edited is
/// open again.
pub fn persist_event(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    vault.transact(|tx| {
        if let Some(event) = tx
            .doc_mut()
            .shared_events
            .iter_mut()
            .find(|event| event.id == id)
        {
            event.archived = false;
        }
        Ok(())
    })
}

pub fn create_shared_event(
    vault: &mut UnlockedVault,
    draft: SharedEventDraft,
) -> Result<SharedEventView> {
    let base = vault.base_and_fx().0;
    let event = plan_create_event(&vault.doc, &base, draft, &now_iso())?;
    let id = event.id.clone();
    vault.transact(|tx| {
        tx.doc_mut().shared_events.push(event);
        Ok(())
    })?;
    vault.shared_event_view(&id)
}

/// Change the event's own fields. Things the purchases and settlements
/// already depend on stay put: the currency, who organizes, whether I take
/// part (while it would strand my payments).
pub fn update_shared_event(
    vault: &mut UnlockedVault,
    id: &str,
    draft: SharedEventDraft,
) -> Result<SharedEventView> {
    let base = vault.base_and_fx().0;
    let event = plan_update_event(&vault.doc, &base, id, draft)?;
    commit_event(vault, event)
}

/// Delete an event. Every real operation it made or adopted stays in the
/// vault as an ordinary one: my payments are plain payments, and the
/// `Collect`/`Repay` transfers are plain debt transfers (they count in the
/// contact balances again, as the event's own balance goes away).
pub fn delete_shared_event(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    find_event(&vault.doc, id)?;
    vault.transact(|tx| {
        tx.doc_mut().shared_events.retain(|known| known.id != id);
        Ok(())
    })
}

/// Put a settled event away (or take it back).
pub fn archive_shared_event(vault: &mut UnlockedVault, id: &str, archived: bool) -> Result<()> {
    check_archivable(&vault.doc, id, archived)?;
    vault.transact(|tx| {
        let event = tx
            .doc_mut()
            .shared_events
            .iter_mut()
            .find(|known| known.id == id)
            .ok_or_else(|| Error::Missing(format!("shared event {id}")))?;
        event.archived = archived;
        Ok(())
    })
}

pub fn add_shared_participant(
    vault: &mut UnlockedVault,
    event_id: &str,
    draft: SharedParticipantDraft,
) -> Result<SharedEventView> {
    let event = plan_add_participant(&vault.doc, event_id, draft)?;
    commit_event(vault, event)
}

/// Rename a guest, change their details, or link them to a contact
/// (their debts then show in the contact's card).
pub fn update_shared_participant(
    vault: &mut UnlockedVault,
    event_id: &str,
    participant_id: &str,
    draft: SharedParticipantDraft,
) -> Result<SharedEventView> {
    let event = plan_update_participant(&vault.doc, event_id, participant_id, draft)?;
    commit_event(vault, event)
}

/// Take someone out of the event; refused once money or a hand-set total
/// mentions them.
pub fn remove_shared_participant(
    vault: &mut UnlockedVault,
    event_id: &str,
    participant_id: &str,
) -> Result<SharedEventView> {
    let event = plan_remove_participant(&vault.doc, event_id, participant_id)?;
    commit_event(vault, event)
}

/// "Make a contact": a guest becomes a contact of the directory with
/// the details they had, and is linked to it.
pub fn make_shared_participant_contact(
    vault: &mut UnlockedVault,
    event_id: &str,
    participant_id: &str,
) -> Result<Contact> {
    let contact_id = crate::service::sync_ids::guest_contact_id(event_id, participant_id);
    let plan = plan_guest_contact(
        &vault.doc,
        event_id,
        participant_id,
        &contact_id,
        &now_iso(),
    )?;
    let contact = plan.contact.clone();
    vault.transact(|tx| {
        if plan.created {
            tx.doc_mut().contacts.push(plan.contact);
        }
        store_event(tx, plan.event)
    })?;
    Ok(contact)
}

/// Fix a person's total by hand (the rest is spread over the others), or
/// take the fixed total back with `None`.
pub fn set_shared_share_override(
    vault: &mut UnlockedVault,
    input: SetShareOverrideInput,
) -> Result<SharedEventView> {
    let event = plan_share_override(&vault.doc, input)?;
    commit_event(vault, event)
}

pub fn add_shared_item(
    vault: &mut UnlockedVault,
    event_id: &str,
    draft: SharedItemDraft,
) -> Result<SharedEventView> {
    let event = plan_add_item(&vault.doc, event_id, draft)?;
    commit_event(vault, event)
}

pub fn update_shared_item(
    vault: &mut UnlockedVault,
    event_id: &str,
    item_id: &str,
    draft: SharedItemDraft,
) -> Result<SharedEventView> {
    let event = plan_update_item(&vault.doc, event_id, item_id, draft)?;
    commit_event(vault, event)
}

/// Delete an item nothing was bought for.
pub fn delete_shared_item(
    vault: &mut UnlockedVault,
    event_id: &str,
    item_id: &str,
) -> Result<SharedEventView> {
    let event = plan_delete_item(&vault.doc, event_id, item_id)?;
    commit_event(vault, event)
}

/// Who shares an item and how.
pub fn set_shared_item_split(
    vault: &mut UnlockedVault,
    input: SetSharedItemSplitInput,
) -> Result<SharedEventView> {
    let event = plan_item_split(&vault.doc, input)?;
    commit_event(vault, event)
}

/// Add a settlement to its event.
fn record_settlement(
    vault: &mut UnlockedVault,
    event_id: &str,
    settlement: Settlement,
) -> Result<SharedEventView> {
    let mut event = find_event(&vault.doc, event_id)?.clone();
    event.settlements.push(settlement);
    commit_event(vault, event)
}

/// Validate the settlement's operation and put it in the document, without
/// saving: the one `transact` that records the settlement writes it too.
fn append_transfer(vault: &mut UnlockedVault, mut transfer: Transaction) -> Result<String> {
    vault.prepare_transaction(&mut transfer)?;
    let id = transfer.meta().id.clone();
    if id.trim().is_empty()
        || vault
            .doc
            .transactions
            .iter()
            .any(|known| known.meta().id == id)
    {
        transfer.meta_mut().id = uuid::Uuid::new_v4().to_string();
    }
    let id = transfer.meta().id.clone();
    vault.doc.transactions.push(transfer);
    vault.recompute_holdings();
    Ok(id)
}

/// Money between a participant and the organizer (or a set-off).
pub fn settle_shared(
    vault: &mut UnlockedVault,
    input: SettleSharedInput,
) -> Result<SharedEventView> {
    let plan = plan_settle(&vault.doc, &input, &now_iso())?;
    let mut settlement = plan.settlement;
    if let Some(transfer) = plan.transfer {
        let tx_id = append_transfer(vault, transfer)?;
        settlement.kind = crate::model::shared_event::SettlementKind::Transfer;
        settlement.tx_id = Some(tx_id);
    }
    record_settlement(vault, &input.event_id, settlement)
}

/// "Write off the rest": I give up what a participant owes me; it becomes my
/// expense in the category of the event.
pub fn write_off_shared(
    vault: &mut UnlockedVault,
    input: SharedRemainderInput,
) -> Result<SharedEventView> {
    let settlement = plan_write_off(&vault.doc, &input, &now_iso())?;
    record_settlement(vault, &input.event_id, settlement)
}

/// "Move onto the others": what a participant has not returned is split
/// between the others.
pub fn reassign_shared(
    vault: &mut UnlockedVault,
    input: SharedRemainderInput,
) -> Result<SharedEventView> {
    let settlement = plan_reassign(&vault.doc, &input, &now_iso())?;
    record_settlement(vault, &input.event_id, settlement)
}

/// Take a settlement back, with the operation it made.
pub fn delete_shared_settlement(
    vault: &mut UnlockedVault,
    event_id: &str,
    settlement_id: &str,
) -> Result<SharedEventView> {
    let tx_id = plan_take_back(&vault.doc, event_id, settlement_id)?;
    if let Some(tx_id) = tx_id {
        vault.drop_shared_tx(&tx_id);
        vault.recompute_holdings();
    }
    let mut event = find_event(&vault.doc, event_id)?.clone();
    event
        .settlements
        .retain(|settlement| settlement.id != settlement_id);
    commit_event(vault, event)
}

/// The due date of a contact participant's debt (the existing debt terms,
/// keyed by contact and currency); an empty date clears it.
pub fn set_shared_due(
    vault: &mut UnlockedVault,
    input: SetSharedDueInput,
) -> Result<Option<DebtTerms>> {
    let draft = plan_due(&vault.doc, input)?;
    vault.set_debt_terms(draft)
}
