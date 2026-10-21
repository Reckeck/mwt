use tauri::{AppHandle, State};

use trade_core::Amount;
use trade_core::calendar::now_secs;
use trade_core::ipc_contract::SetManualPriceInput;
use trade_core::model::planning::AssetClass;
use trade_core::model::store::AssetRef;
use trade_core::model::vault::ManualPrice;
use trade_core::service::{
    AssetClassTarget, AssetClassesView, AssetHoldings, HistoryPoint, NetWorth, PortfolioSummary,
    RealizedPnl, StoreMonthRow, TopupAdvice,
};

use crate::commands::app_settings::current_zone;
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

/// The same asset on every wallet (the «elsewhere» block of the asset sheet).
#[tauri::command]
pub async fn get_asset_holdings(
    asset: AssetRef,
    state: State<'_, AppState>,
) -> Result<AssetHoldings, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().asset_holdings(&asset))
}

/// Money in and out of a wallet per local month, the last `months` (default 6).
#[tauri::command]
pub async fn get_store_months(
    app: AppHandle,
    store_id: String,
    include_children: Option<bool>,
    months: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<StoreMonthRow>, String> {
    let zone = current_zone(&app);
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().store_months(
        &store_id,
        include_children.unwrap_or(true),
        months.unwrap_or(6).clamp(1, 36),
        now_secs(),
        &zone,
    ))
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

/// Portfolio baskets with target shares; the preset until saved (plan F).
#[tauri::command]
pub async fn asset_classes_get(state: State<'_, AppState>) -> Result<AssetClassesView, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().asset_classes())
}

/// Replace the baskets; shares must add up to 100, an empty list = preset.
#[tauri::command]
pub async fn asset_classes_set(
    classes: Vec<AssetClass>,
    state: State<'_, AppState>,
) -> Result<AssetClassesView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .set_asset_classes(classes)
        .map_err(err_to_string)
}

/// Put an asset or a store in a basket, or take it out with `None`.
#[tauri::command]
pub async fn asset_class_assign(
    target: AssetClassTarget,
    class_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<AssetClassesView, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .assign_asset_class(target, class_id)
        .map_err(err_to_string)
}

/// Where the free money should go to bring the shares back to target.
#[tauri::command]
pub async fn topup_advice(
    app: AppHandle,
    amount: Option<Amount>,
    state: State<'_, AppState>,
) -> Result<TopupAdvice, String> {
    let zone = current_zone(&app);
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard
        .as_ref()
        .unwrap()
        .topup_advice(amount, now_secs(), &zone))
}
