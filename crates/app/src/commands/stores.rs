use tauri::State;

use trade_core::ipc_contract::{CreateStoreInput, UpdateStoreInput};
use trade_core::model::store::Store;
use trade_core::service::StoreOverview;

use crate::commands::{AppState, err_to_string, require_vault};

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
    vault
        .create_store(
            &input.name,
            input.kind,
            &input.color,
            &input.note,
            input.parent_id,
        )
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
            input.parent_id,
        )
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
/// positions and realized PnL.
#[tauri::command]
pub async fn get_store_overview(
    store_id: String,
    state: State<'_, AppState>,
) -> Result<StoreOverview, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .store_overview(&store_id)
        .map_err(err_to_string)
}
