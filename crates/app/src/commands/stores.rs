use tauri::State;

use trade_core::ipc_contract::{
    CreateStoreInput, FundEnvelopeInput, MoveEnvelopeInput, MoveStoreInput, UpdateEnvelopeInput,
    UpdateStoreInput, WalletCatalog,
};
use trade_core::model::planning::Envelope;
use trade_core::model::store::{Store, StoreKind};
use trade_core::service::{
    EnvelopeDraft, EnvelopeView, StoreExtras, StoreMoveTargets, StoreOverview, StoreTreeNode,
};

use crate::commands::{AppState, err_to_string, require_vault};

/// What every wallet kind allows (sub-account types, limits, exchanges).
/// Pure: needs no vault.
#[tauri::command]
pub async fn get_wallet_catalog() -> Result<WalletCatalog, String> {
    Ok(WalletCatalog::build())
}

#[tauri::command]
pub async fn list_stores(state: State<'_, AppState>) -> Result<Vec<Store>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().doc.stores.clone())
}

#[tauri::command]
pub async fn create_store(
    input: CreateStoreInput,
    state: State<'_, AppState>,
) -> Result<Store, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    let extras = StoreExtras {
        purpose: input.purpose,
        liability: input.liability,
        contact_id: input.contact_id,
        sub_type: input.sub_type,
        binding: input.binding,
        custody: input.custody,
        property: input.property,
        secured_by: input.secured_by,
        provider: input.provider,
        opening: input.opening,
    };
    match input.card {
        Some(card) if input.kind == StoreKind::Bank && input.parent_id.is_none() => {
            vault.create_bank_with_card(&input.name, &input.color, &input.note, extras, card)
        }
        _ => vault.create_store_with(
            &input.name,
            input.kind,
            &input.color,
            &input.note,
            input.parent_id,
            extras,
        ),
    }
    .map_err(err_to_string)
}

#[tauri::command]
pub async fn update_store(
    input: UpdateStoreInput,
    state: State<'_, AppState>,
) -> Result<Store, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    vault
        .update_store(
            &input.id,
            &input.name,
            &input.color,
            &input.note,
            StoreExtras {
                purpose: input.purpose,
                liability: input.liability,
                contact_id: input.contact_id,
                sub_type: None,
                binding: input.binding,
                custody: input.custody,
                property: input.property,
                secured_by: input.secured_by,
                provider: input.provider,
                opening: None,
            },
        )
        .map_err(err_to_string)
}

/// The wallet tree with own and rolled-up values (sidebar, wallets page).
#[tauri::command]
pub async fn get_store_tree(state: State<'_, AppState>) -> Result<Vec<StoreTreeNode>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().store_tree())
}

#[tauri::command]
pub async fn move_store(
    input: MoveStoreInput,
    state: State<'_, AppState>,
) -> Result<Store, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    vault
        .move_store(&input.id, input.parent_id.as_deref())
        .map_err(err_to_string)
}

/// Where a sub-account may be moved to.
#[tauri::command]
pub async fn store_move_targets(
    id: String,
    state: State<'_, AppState>,
) -> Result<StoreMoveTargets, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .store_move_targets(&id)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn archive_store(
    id: String,
    archived: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    vault.archive_store(&id, archived).map_err(err_to_string)
}

#[tauri::command]
pub async fn delete_store(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    vault.delete_store(&id).map_err(err_to_string)
}

/// One round trip for the whole wallet page header: store card, totals,
/// positions and realized PnL. `include_children` (default on) widens PnL and
/// transaction stats to the subtree.
#[tauri::command]
pub async fn get_store_overview(
    store_id: String,
    include_children: Option<bool>,
    state: State<'_, AppState>,
) -> Result<StoreOverview, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .store_overview(&store_id, include_children.unwrap_or(true))
        .map_err(err_to_string)
}

// -- envelopes ---------------------------------------------------------------

/// Envelopes of one store, or of every store when `store_id` is absent.
#[tauri::command]
pub async fn list_envelopes(
    store_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<EnvelopeView>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().envelopes(store_id.as_deref()))
}

#[tauri::command]
pub async fn create_envelope(
    input: EnvelopeDraft,
    state: State<'_, AppState>,
) -> Result<Envelope, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .create_envelope(input)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn update_envelope(
    input: UpdateEnvelopeInput,
    state: State<'_, AppState>,
) -> Result<Envelope, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_envelope(&input.id, input.draft)
        .map_err(err_to_string)
}

/// Earmark more (`amount > 0`) or release (`amount < 0`).
#[tauri::command]
pub async fn fund_envelope(
    input: FundEnvelopeInput,
    state: State<'_, AppState>,
) -> Result<Envelope, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .fund_envelope(&input.id, input.amount)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn move_between_envelopes(
    input: MoveEnvelopeInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .move_between_envelopes(&input.from, &input.to, input.amount)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn delete_envelope(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_envelope(&id)
        .map_err(err_to_string)
}
