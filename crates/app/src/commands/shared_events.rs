//! Shared events ("shared events"): thin wrappers over the vault's event service.
//! Every mutation answers with the refreshed event (`SharedEventView`) so the
//! screen can redraw from the reply alone.

use tauri::State;

use trade_core::household::shared_events::view::{self as event_views};
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
use trade_core::usecases::household::shared_events;

use crate::commands::AppState;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// The events, newest first; archived ones only on request.
#[tauri::command]
pub async fn list_shared_events(
    filter: Option<SharedEventFilter>,
    state: State<'_, AppState>,
) -> Result<Vec<SharedEventSummary>, IpcError> {
    let include_archived = filter.is_some_and(|filter| filter.include_archived);
    with_vault(&state, |vault| {
        Ok(event_views::list_shared_events(
            &vault.doc,
            include_archived,
        ))
    })
}

/// One event with everything worked out: shares, balances, plan of payments.
#[tauri::command]
pub async fn get_shared_event(
    id: String,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault(&state, |vault| {
        event_views::shared_event_view(&vault.doc, &id)
    })
}

#[tauri::command]
pub async fn create_shared_event(
    input: SharedEventDraft,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::create_shared_event(vault, input)
    })
}

#[tauri::command]
pub async fn update_shared_event(
    input: UpdateSharedEventInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::update_shared_event(vault, &input.id, input.event)
    })
}

/// Deletes the event and the payments it made; adopted payments stay.
#[tauri::command]
pub async fn delete_shared_event(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::delete_shared_event(vault, &id)
    })
}

#[tauri::command]
pub async fn archive_shared_event(
    input: ArchiveSharedEventInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::archive_shared_event(vault, &input.id, input.archived)
    })
}

#[tauri::command]
pub async fn add_shared_participant(
    input: SharedParticipantInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::add_shared_participant(vault, &input.event_id, input.participant)
    })
}

/// Rename a guest, change their details, or link them to a contact.
#[tauri::command]
pub async fn update_shared_participant(
    input: UpdateSharedParticipantInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::update_shared_participant(
            vault,
            &input.event_id,
            &input.participant_id,
            input.participant,
        )
    })
}

#[tauri::command]
pub async fn remove_shared_participant(
    input: SharedParticipantRef,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::remove_shared_participant(vault, &input.event_id, &input.participant_id)
    })
}

/// "Make a contact": the guest becomes a contact with their details.
#[tauri::command]
pub async fn make_shared_participant_contact(
    input: SharedParticipantRef,
    state: State<'_, AppState>,
) -> Result<Contact, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::make_shared_participant_contact(
            vault,
            &input.event_id,
            &input.participant_id,
        )
    })
}

#[tauri::command]
pub async fn add_shared_item(
    input: SharedItemInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::add_shared_item(vault, &input.event_id, input.item)
    })
}

#[tauri::command]
pub async fn update_shared_item(
    input: UpdateSharedItemInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::update_shared_item(vault, &input.event_id, &input.item_id, input.item)
    })
}

#[tauri::command]
pub async fn delete_shared_item(
    input: SharedItemRef,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::delete_shared_item(vault, &input.event_id, &input.item_id)
    })
}

/// Who shares an item: everybody, some people, or by units.
#[tauri::command]
pub async fn set_shared_item_split(
    input: SetSharedItemSplitInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::set_shared_item_split(vault, input)
    })
}

/// A real purchase; mine also makes a payment in the chosen wallet.
#[tauri::command]
pub async fn add_shared_purchase(
    input: AddSharedPurchaseInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::add_shared_purchase(vault, &input.event_id, input.purchase)
    })
}

#[tauri::command]
pub async fn update_shared_purchase(
    input: UpdateSharedPurchaseInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::update_shared_purchase(
            vault,
            &input.event_id,
            &input.purchase_id,
            input.purchase,
        )
    })
}

#[tauri::command]
pub async fn delete_shared_purchase(
    input: SharedPurchaseRef,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::delete_shared_purchase(vault, &input.event_id, &input.purchase_id)
    })
}

/// Take a payment that is already in the vault as a purchase of the event.
#[tauri::command]
pub async fn attach_payment_to_shared_event(
    input: AttachPaymentInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::attach_payment_to_shared_event(vault, input)
    })
}

/// A hand-set total for one person; `total: null` takes it back.
#[tauri::command]
pub async fn set_shared_share_override(
    input: SetShareOverrideInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::set_shared_share_override(vault, input)
    })
}

/// "Received" / "Gave" / "Pay my share" / "Set off without an operation".
#[tauri::command]
pub async fn settle_shared(
    input: SettleSharedInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| shared_events::settle_shared(vault, input))
}

/// "Write off the rest": the rest becomes my expense.
#[tauri::command]
pub async fn write_off_shared(
    input: SharedRemainderInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::write_off_shared(vault, input)
    })
}

/// "Move onto the others": the rest is split between the others.
#[tauri::command]
pub async fn reassign_shared(
    input: SharedRemainderInput,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| shared_events::reassign_shared(vault, input))
}

/// Take a settlement back (the toast's "Undo"), with its operation.
#[tauri::command]
pub async fn delete_shared_settlement(
    input: SharedSettlementRef,
    state: State<'_, AppState>,
) -> Result<SharedEventView, IpcError> {
    with_vault_mut(&state, |vault| {
        shared_events::delete_shared_settlement(vault, &input.event_id, &input.settlement_id)
    })
}

/// The term of a contact participant's debt (the usual debt terms).
#[tauri::command]
pub async fn set_shared_due(
    input: SetSharedDueInput,
    state: State<'_, AppState>,
) -> Result<Option<DebtTerms>, IpcError> {
    with_vault_mut(&state, |vault| shared_events::set_shared_due(vault, input))
}

/// The settlement as text for a chat.
#[tauri::command]
pub async fn get_shared_event_share_text(
    input: SharedShareTextInput,
    state: State<'_, AppState>,
) -> Result<String, IpcError> {
    with_vault(&state, |vault| {
        shared_events::shared_event_share_text(vault, &input.event_id, &input.locale)
    })
}

/// What the events say about one contact: the "Shared events" block.
#[tauri::command]
pub async fn get_contact_shared_events(
    contact_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ContactEventBalance>, IpcError> {
    with_vault(&state, |vault| {
        Ok(event_views::contact_shared_events(&vault.doc, &contact_id))
    })
}

/// Operations of the vault that an event made or adopted, to mark them in
/// the ledger and to keep a payment from joining two events.
#[tauri::command]
pub async fn list_shared_tx_links(
    state: State<'_, AppState>,
) -> Result<Vec<SharedTxLink>, IpcError> {
    with_vault(&state, |vault| Ok(event_views::shared_tx_links(&vault.doc)))
}

ipc_commands!(shared_events_commands, SHARED_EVENTS_NAMES, commands::shared_events: [list_shared_events, get_shared_event, create_shared_event, update_shared_event, delete_shared_event, archive_shared_event, add_shared_participant, update_shared_participant, remove_shared_participant, make_shared_participant_contact, add_shared_item, update_shared_item, delete_shared_item, set_shared_item_split, add_shared_purchase, update_shared_purchase, delete_shared_purchase, attach_payment_to_shared_event, set_shared_share_override, settle_shared, write_off_shared, reassign_shared, delete_shared_settlement, set_shared_due, get_shared_event_share_text, get_contact_shared_events, list_shared_tx_links]);
