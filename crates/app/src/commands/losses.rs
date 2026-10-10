//! Losses, chains and the rates table (plan §6.3–§6.4). The loss report
//! itself is `get_report` with kind `losses`.

use std::collections::BTreeMap;

use tauri::State;

use trade_core::ipc_contract::LinkTransactionsInput;
use trade_core::service::{FlowSummary, FxTable, TxLoss};

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn get_tx_loss(id: String, state: State<'_, AppState>) -> Result<TxLoss, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard.as_ref().unwrap().tx_loss(&id).map_err(err_to_string)
}

/// Every transaction with a loss, by id, in the base currency.
#[tauri::command]
pub async fn get_tx_losses(state: State<'_, AppState>) -> Result<BTreeMap<String, TxLoss>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().tx_losses())
}

/// Chains with a link in `[from, to)`, RFC3339 bounds; newest first.
#[tauri::command]
pub async fn list_flows(
    from: Option<String>,
    to: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<FlowSummary>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard
        .as_ref()
        .unwrap()
        .list_flows(from.as_deref(), to.as_deref()))
}

#[tauri::command]
pub async fn link_transactions(
    input: LinkTransactionsInput,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .link_transactions(&input.ids, input.flow_id.as_deref())
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn unlink_transaction(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .unlink_transaction(&id)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn get_fx_table(state: State<'_, AppState>) -> Result<FxTable, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().fx_table())
}
