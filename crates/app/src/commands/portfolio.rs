use tauri::State;

use trade_core::ipc_contract::SetManualPriceInput;
use trade_core::model::vault::ManualPrice;
use trade_core::service::{HistoryPoint, NetWorth, PortfolioSummary, RealizedPnl};

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn get_summary(state: State<'_, AppState>) -> Result<PortfolioSummary, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().portfolio_summary())
}

#[tauri::command]
pub async fn get_history(
    base_currency: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryPoint>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    let base = base_currency.unwrap_or_else(|| vault.base_currency().to_string());
    Ok(vault.history_series(&base))
}

#[tauri::command]
pub async fn get_realized_pnl(state: State<'_, AppState>) -> Result<RealizedPnl, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().realized_pnl())
}

/// Value of a single store over time (wallet page chart); with its
/// sub-accounts unless `include_children` is `false`.
#[tauri::command]
pub async fn get_store_history(
    store_id: String,
    include_children: Option<bool>,
    base_currency: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryPoint>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    let base = base_currency.unwrap_or_else(|| vault.base_currency().to_string());
    Ok(vault.store_history(&store_id, include_children.unwrap_or(true), &base))
}

/// Realized PnL booked on a single store (and, by default, its sub-accounts).
#[tauri::command]
pub async fn get_store_pnl(
    store_id: String,
    include_children: Option<bool>,
    state: State<'_, AppState>,
) -> Result<RealizedPnl, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard
        .as_ref()
        .unwrap()
        .store_pnl(&store_id, include_children.unwrap_or(true)))
}

/// Assets, debts and what people owe, in the base currency.
#[tauri::command]
pub async fn get_net_worth(state: State<'_, AppState>) -> Result<NetWorth, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().net_worth())
}

/// Hand-set value of one unit of a property asset (a flat, a car).
#[tauri::command]
pub async fn set_manual_price(
    input: SetManualPriceInput,
    state: State<'_, AppState>,
) -> Result<ManualPrice, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .set_manual_price(&input.asset_key, input.unit_price, &input.currency)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn clear_manual_price(
    asset_key: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .clear_manual_price(&asset_key)
        .map_err(err_to_string)
}
