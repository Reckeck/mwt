//! Scenarios of the shared events area (the event, its people and items,
//! purchases, settling, the ledger and the budget figures): each decides in
//! `household::shared_events`, then ends in one `vault.transact`. A settlement
//! or a purchase that makes a real operation writes the operation first through
//! the transactions area, then records itself in the event.

use chrono::{Datelike, NaiveDate};

use crate::calendar::Calendar;
use crate::error::{Error, Result};
use crate::household::shared_events::analytics::{category_group, extras_between, value_in};
use crate::household::shared_events::ledger::{apply_edit, cascade_delete};
use crate::household::shared_events::purchases::{
    PaymentStep, PurchaseEdit, link_purchase, plan_add_purchase, plan_attach_payment,
    plan_delete_purchase, plan_update_purchase,
};
use crate::household::shared_events::settle::{
    plan_due, plan_reassign, plan_settle, plan_take_back, plan_write_off,
};
use crate::household::shared_events::text::share_text;
use crate::household::shared_events::{
    check_archivable, find_event, plan_add_item, plan_add_participant, plan_create_event,
    plan_delete_item, plan_guest_contact, plan_item_split, plan_remove_participant,
    plan_share_override, plan_update_event, plan_update_item, plan_update_participant,
};
use crate::ipc_contract::{
    AttachPaymentInput, SetShareOverrideInput, SetSharedDueInput, SetSharedItemSplitInput,
    SettleSharedInput, SharedEventDraft, SharedItemDraft, SharedParticipantDraft,
    SharedPurchaseDraft, SharedRemainderInput,
};
use crate::kernel::tx::Tx;
use crate::model::VaultDoc;
use crate::model::contact::{Contact, DebtTerms};
use crate::model::shared_event::{Settlement, SharedEvent};
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::money::rates::RateBook;
use crate::money::shared_ratios::SharedExtra;
use crate::service::{FlowClass, SharedEventView, UnlockedVault};
use crate::usecases::planning::budget::Flow;

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
        drop_shared_tx(vault, &tx_id);
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

/// Remove a transaction without saving; the caller saves.
fn drop_shared_tx(vault: &mut UnlockedVault, tx_id: &str) {
    vault.doc.transactions.retain(|tx| tx.meta().id != tx_id);
    vault.set_property_link(tx_id, None);
}

/// Do what a changed purchase asks of the vault's payments, then write the
/// event. Nothing of the event is saved if the vault refuses the payment.
fn commit_purchase(vault: &mut UnlockedVault, edit: PurchaseEdit) -> Result<SharedEventView> {
    let PurchaseEdit { mut event, payment } = edit;
    match payment {
        PaymentStep::Keep => {}
        PaymentStep::Add {
            payment,
            purchase_id,
        } => {
            let saved = vault.add_transaction(payment)?;
            link_purchase(&mut event, &purchase_id, &saved.meta().id);
        }
        PaymentStep::Update { tx_id, payment } => {
            vault.update_transaction_unsynced(&tx_id, payment)?;
        }
        PaymentStep::Drop { tx_id } => {
            drop_shared_tx(vault, &tx_id);
            vault.recompute_holdings();
        }
    }
    commit_event(vault, event)
}

pub fn add_shared_purchase(
    vault: &mut UnlockedVault,
    event_id: &str,
    draft: SharedPurchaseDraft,
) -> Result<SharedEventView> {
    let books = vault.books();
    let edit = plan_add_purchase(books.rate_book(), books.doc(), event_id, &draft, &now_iso())?;
    commit_purchase(vault, edit)
}

/// Change a purchase; its item stays. A payment the event made follows the
/// change; an adopted payment keeps its own amount and only the number of
/// units can change.
pub fn update_shared_purchase(
    vault: &mut UnlockedVault,
    event_id: &str,
    purchase_id: &str,
    draft: SharedPurchaseDraft,
) -> Result<SharedEventView> {
    let books = vault.books();
    let edit = plan_update_purchase(
        books.rate_book(),
        books.doc(),
        event_id,
        purchase_id,
        &draft,
        &now_iso(),
    )?;
    commit_purchase(vault, edit)
}

/// Delete a purchase with the payment the event made for it; an adopted
/// payment stays in the vault, unlinked.
pub fn delete_shared_purchase(
    vault: &mut UnlockedVault,
    event_id: &str,
    purchase_id: &str,
) -> Result<SharedEventView> {
    let edit = plan_delete_purchase(&vault.doc, event_id, purchase_id)?;
    commit_purchase(vault, edit)
}

/// Take a payment that is already in the vault as my purchase of the event.
pub fn attach_payment_to_shared_event(
    vault: &mut UnlockedVault,
    input: AttachPaymentInput,
) -> Result<SharedEventView> {
    let purchase_id = crate::service::sync_ids::payment_link_id(&input.tx_id);
    let books = vault.books();
    let event = plan_attach_payment(books.rate_book(), books.doc(), &input, purchase_id)?;
    commit_event(vault, event)
}

/// Drop the purchases and settlements of every event that point at an
/// operation about to be deleted. Reopens an archived event. The caller saves.
pub(crate) fn cascade_shared_delete(vault: &mut UnlockedVault, tx_id: &str) {
    cascade_delete(&mut vault.doc.shared_events, tx_id);
}

/// Put the event a ledger edit planned in place of its older self. The caller
/// saves.
pub(crate) fn apply_shared_edit(vault: &mut UnlockedVault, next: Option<SharedEvent>) {
    apply_edit(&mut vault.doc.shared_events, next);
}

/// The settlement as text for a chat, in `ru` or `en`.
pub fn shared_event_share_text(
    vault: &UnlockedVault,
    event_id: &str,
    locale: &str,
) -> Result<String> {
    share_text(&vault.doc, event_id, locale, chrono::Utc::now().year())
}

/// One spending flow of mine in the event currency, valued in `currency`.
fn event_flow(
    doc: &VaultDoc,
    book: &RateBook,
    cal: &Calendar,
    currency: &str,
    extra: &SharedExtra,
) -> Flow {
    let (value, missing) = value_in(book, extra, currency);
    let date: NaiveDate = cal.date_of(extra.secs);
    Flow {
        date,
        class: UnlockedVault::spending(category_group(doc, &extra.category)),
        value,
        categories: vec![extra.category.clone()],
        stores: Vec::new(),
        tx_id: extra.tag.clone(),
        missing,
    }
}

/// Spending that has no payment of mine behind it: my part of other
/// people's purchases and what I wrote off, over entries the caller already
/// collected (`Books` keeps them, so a request collects them once).
pub(crate) fn extra_flows_from(
    doc: &VaultDoc,
    extras: &[SharedExtra],
    book: &RateBook,
    cal: &Calendar,
    currency: &str,
    from: i64,
    to: i64,
) -> Vec<Flow> {
    extras_between(extras, from, to)
        .map(|extra| event_flow(doc, book, cal, currency, extra))
        .filter(|flow| flow.class != FlowClass::Ignored)
        .collect()
}
