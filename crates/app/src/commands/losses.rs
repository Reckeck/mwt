//! Losses, chains and the rates table (plan §6.3–§6.4). The loss report
//! itself is `get_report` with kind `losses`.

use std::collections::BTreeMap;

use tauri::{AppHandle, State};

use trade_core::ipc_contract::LinkTransactionsInput;
use trade_core::service::{FlowSummary, FxTable, TxLoss};

use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

#[tauri::command]
pub async fn get_tx_loss(id: String, state: State<'_, AppState>) -> Result<TxLoss, IpcError> {
    let guard = require_vault(&state)?;
    guard.as_ref().unwrap().tx_loss(&id).map_err(IpcError::from)
}

/// Every transaction with a loss, by id, in the base currency.
#[tauri::command]
pub async fn get_tx_losses(
    state: State<'_, AppState>,
) -> Result<BTreeMap<String, TxLoss>, IpcError> {
    let guard = require_vault(&state)?;
    Ok(guard.as_ref().unwrap().tx_losses())
}

/// Chains with a link in `[from, to)`, RFC3339 bounds; newest first.
#[tauri::command]
pub async fn list_flows(
    from: Option<String>,
    to: Option<String>,
    store_ids: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<Vec<FlowSummary>, IpcError> {
    let guard = require_vault(&state)?;
    Ok(guard.as_ref().unwrap().list_flows(
        from.as_deref(),
        to.as_deref(),
        &store_ids.unwrap_or_default(),
    ))
}

#[tauri::command]
pub async fn link_transactions(
    input: LinkTransactionsInput,
    state: State<'_, AppState>,
) -> Result<String, IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .link_transactions(&input.ids, input.flow_id.as_deref())
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn unlink_transaction(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .unlink_transaction(&id)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn get_fx_table(app: AppHandle, state: State<'_, AppState>) -> Result<FxTable, IpcError> {
    // Favourites live outside the vault; coins among them get a row too.
    let favorites = crate::commands::app_settings::current_settings(&app).favorite_currencies;
    let guard = require_vault(&state)?;
    Ok(guard.as_ref().unwrap().fx_table(&favorites))
}

ipc_commands!(losses_commands, LOSSES_NAMES, commands::losses: [get_tx_loss, get_tx_losses, list_flows, link_transactions, unlink_transaction, get_fx_table]);
