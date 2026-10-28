//! Shared events («посиделки»): thin wrappers over the vault's event service.
//! Every mutation answers with the refreshed event (`SharedEventView`) so the
//! screen can redraw from the reply alone.

use tauri::State;

use trade_core::ipc_contract::{
    AddSharedPurchaseInput, ArchiveSharedEventInput, AttachPaymentInput, SetShareOverrideInput,
    SetSharedDueInput, SetSharedItemSplitInput, SettleSharedInput, SharedEventDraft,
    SharedEventFilter, SharedItemInput, SharedItemRef, SharedParticipantInput,
    SharedParticipantRef, SharedPurchaseRef, SharedRemainderInput, SharedSettlementRef,
    SharedShareTextInput, UpdateSharedEventInput, UpdateSharedItemInput,
    UpdateSharedParticipantInput, UpdateSharedPurchaseInput,
};
use trade_core::model::contact::{Contact, DebtTerms};
use trade_core::service::{ContactEventBalance, SharedEventSummary, SharedEventView, SharedTxLink};

use crate::commands::{AppState, err_to_string, require_vault};

/// The events, newest first; archived ones only on request.
#[tauri::command]
pub async fn list_shared_events(
    filter: Option<SharedEventFilter>,
    state: State<'_, AppState>,
) -> Result<Vec<SharedEventSummary>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let include_archived = filter.is_some_and(|filter| filter.include_archived);
    Ok(guard.as_ref().unwrap().list_shared_events(include_archived))
}

/// One event with everything worked out: shares, balances, plan of payments.
#[tauri::command]
pub async fn get_shared_event(
    id: String,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .shared_event_view(&id)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn create_shared_event(
    input: SharedEventDraft,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .create_shared_event(input)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn update_shared_event(
    input: UpdateSharedEventInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_shared_event(&input.id, input.event)
        .map_err(err_to_string)
}

/// Deletes the event and the payments it made; adopted payments stay.
#[tauri::command]
pub async fn delete_shared_event(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_shared_event(&id)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn archive_shared_event(
    input: ArchiveSharedEventInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .archive_shared_event(&input.id, input.archived)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn add_shared_participant(
    input: SharedParticipantInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .add_shared_participant(&input.event_id, input.participant)
        .map_err(err_to_string)
}

/// Rename a guest, change their details, or link them to a contact.
#[tauri::command]
pub async fn update_shared_participant(
    input: UpdateSharedParticipantInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_shared_participant(&input.event_id, &input.participant_id, input.participant)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn remove_shared_participant(
    input: SharedParticipantRef,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .remove_shared_participant(&input.event_id, &input.participant_id)
        .map_err(err_to_string)
}

/// «Сделать контактом»: the guest becomes a contact with their details.
#[tauri::command]
pub async fn make_shared_participant_contact(
    input: SharedParticipantRef,
    state: State<'_, AppState>,
) -> Result<Contact, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .make_shared_participant_contact(&input.event_id, &input.participant_id)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn add_shared_item(
    input: SharedItemInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .add_shared_item(&input.event_id, input.item)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn update_shared_item(
    input: UpdateSharedItemInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_shared_item(&input.event_id, &input.item_id, input.item)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn delete_shared_item(
    input: SharedItemRef,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_shared_item(&input.event_id, &input.item_id)
        .map_err(err_to_string)
}

/// Who shares an item: everybody, some people, or by units.
#[tauri::command]
pub async fn set_shared_item_split(
    input: SetSharedItemSplitInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .set_shared_item_split(input)
        .map_err(err_to_string)
}

/// A real purchase; mine also makes a payment in the chosen wallet.
#[tauri::command]
pub async fn add_shared_purchase(
    input: AddSharedPurchaseInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .add_shared_purchase(&input.event_id, input.purchase)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn update_shared_purchase(
    input: UpdateSharedPurchaseInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_shared_purchase(&input.event_id, &input.purchase_id, input.purchase)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn delete_shared_purchase(
    input: SharedPurchaseRef,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_shared_purchase(&input.event_id, &input.purchase_id)
        .map_err(err_to_string)
}

/// Take a payment that is already in the vault as a purchase of the event.
#[tauri::command]
pub async fn attach_payment_to_shared_event(
    input: AttachPaymentInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .attach_payment_to_shared_event(input)
        .map_err(err_to_string)
}

/// A hand-set total for one person; `total: null` takes it back.
#[tauri::command]
pub async fn set_shared_share_override(
    input: SetShareOverrideInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .set_shared_share_override(input)
        .map_err(err_to_string)
}

/// «Получил» / «Отдал» / «Оплатить долю» / «Зачесть без операции».
#[tauri::command]
pub async fn settle_shared(
    input: SettleSharedInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .settle_shared(input)
        .map_err(err_to_string)
}

/// «Списать остаток»: the rest becomes my expense.
#[tauri::command]
pub async fn write_off_shared(
    input: SharedRemainderInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .write_off_shared(input)
        .map_err(err_to_string)
}

/// «Переложить на других»: the rest is split between the others.
#[tauri::command]
pub async fn reassign_shared(
    input: SharedRemainderInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .reassign_shared(input)
        .map_err(err_to_string)
}

/// Take a settlement back (the toast's «Отменить»), with its operation.
#[tauri::command]
pub async fn delete_shared_settlement(
    input: SharedSettlementRef,
    state: State<'_, AppState>,
) -> Result<SharedEventView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_shared_settlement(&input.event_id, &input.settlement_id)
        .map_err(err_to_string)
}

/// The term of a contact participant's debt (the usual debt terms).
#[tauri::command]
pub async fn set_shared_due(
    input: SetSharedDueInput,
    state: State<'_, AppState>,
) -> Result<Option<DebtTerms>, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .set_shared_due(input)
        .map_err(err_to_string)
}

/// The settlement as text for a chat.
#[tauri::command]
pub async fn get_shared_event_share_text(
    input: SharedShareTextInput,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .shared_event_share_text(&input.event_id, &input.locale)
        .map_err(err_to_string)
}

/// What the events say about one contact: the «Общие события» block.
#[tauri::command]
pub async fn get_contact_shared_events(
    contact_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ContactEventBalance>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().contact_shared_events(&contact_id))
}

/// Operations of the vault that an event made or adopted, to mark them in
/// the ledger and to keep a payment from joining two events.
#[tauri::command]
pub async fn list_shared_tx_links(state: State<'_, AppState>) -> Result<Vec<SharedTxLink>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().shared_tx_links())
}
