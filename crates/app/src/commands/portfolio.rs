use tauri::State;

use trade_core::service::{HistoryPoint, PortfolioSummary, RealizedPnl};

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn get_summary(state: State<'_, AppState>) -> Result<PortfolioSummary, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    Ok(g.as_ref().unwrap().portfolio_summary())
}

#[tauri::command]
pub async fn get_history(
    base_currency: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryPoint>, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_ref().unwrap();
    let base = base_currency.unwrap_or_else(|| v.doc.settings.base_currency.clone());
    Ok(v.history_series(&base))
}

#[tauri::command]
pub async fn get_realized_pnl(state: State<'_, AppState>) -> Result<RealizedPnl, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    Ok(g.as_ref().unwrap().realized_pnl())
}

/// Value of a single store over time (wallet page chart).
#[tauri::command]
pub async fn get_store_history(
    store_id: String,
    base_currency: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryPoint>, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_ref().unwrap();
    let base = base_currency.unwrap_or_else(|| v.doc.settings.base_currency.clone());
    Ok(v.store_history(&store_id, &base))
}

/// Realized PnL booked on a single store.
#[tauri::command]
pub async fn get_store_pnl(
    store_id: String,
    state: State<'_, AppState>,
) -> Result<RealizedPnl, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    Ok(g.as_ref().unwrap().store_pnl(&store_id))
}
