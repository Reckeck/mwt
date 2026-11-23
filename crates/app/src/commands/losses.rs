//! Losses, chains and the rates table (plan §6.3–§6.4). The loss report
//! itself is `get_report` with kind `losses`.

use std::collections::BTreeMap;

use tauri::{AppHandle, State};

use trade_core::household::losses::{FlowSummary, FxTable, TxLoss};
use trade_core::ipc_contract::LinkTransactionsInput;
use trade_core::usecases::household::losses;

use crate::commands::AppState;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

#[tauri::command]
pub async fn get_tx_loss(id: String, state: State<'_, AppState>) -> Result<TxLoss, IpcError> {
    with_vault(&state, |vault| losses::tx_loss(vault, &id))
}

/// Every transaction with a loss, by id, in the base currency.
#[tauri::command]
pub async fn get_tx_losses(
    state: State<'_, AppState>,
) -> Result<BTreeMap<String, TxLoss>, IpcError> {
    with_vault(&state, |vault| Ok(losses::tx_losses(vault)))
}

/// Chains with a link in `[from, to)`, RFC3339 bounds; newest first.
#[tauri::command]
pub async fn list_flows(
    from: Option<String>,
    to: Option<String>,
    store_ids: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<Vec<FlowSummary>, IpcError> {
    with_vault(&state, |vault| {
        Ok(losses::list_flows(
            vault,
            from.as_deref(),
            to.as_deref(),
            &store_ids.unwrap_or_default(),
        ))
    })
}

#[tauri::command]
pub async fn link_transactions(
    input: LinkTransactionsInput,
    state: State<'_, AppState>,
) -> Result<String, IpcError> {
    with_vault_mut(&state, |vault| {
        losses::link_transactions(vault, &input.ids, input.flow_id.as_deref())
    })
}

#[tauri::command]
pub async fn unlink_transaction(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| losses::unlink_transaction(vault, &id))
}

#[tauri::command]
pub async fn get_fx_table(app: AppHandle, state: State<'_, AppState>) -> Result<FxTable, IpcError> {
    // Favourites live outside the vault; coins among them get a row too.
    let favorites = crate::commands::app_settings::current_settings(&app).favorite_currencies;
    with_vault(&state, |vault| Ok(losses::fx_table(vault, &favorites)))
}

ipc_commands!(losses_commands, LOSSES_NAMES, commands::losses: [get_tx_loss, get_tx_losses, list_flows, link_transactions, unlink_transaction, get_fx_table]);
