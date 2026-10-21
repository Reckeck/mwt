//! Inflation and the key rate from the statistics offices: the background
//! refresh, the «Обновить» button and the per-currency mode.

use chrono::{Datelike, Months, NaiveDate};
use tauri::{AppHandle, Emitter, Manager, State};
use trade_core::model::inflation::{InflationMode, InflationSource};
use trade_core::pricing::PricingService;
use trade_core::service::{FetchedRates, InflationCurrency};

use crate::commands::app_settings::current_settings;
use crate::commands::{AppState, err_to_string, require_vault};

/// Months of inflation asked for: the chart's two years plus a spare.
const INFLATION_MONTHS_BACK: u32 = 25;

/// Everything every source answers; a source that fails is just left out.
async fn fetch_rates(pricing: &PricingService, today: NaiveDate) -> FetchedRates {
    let key_from = NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap_or(today);
    let key_rates = pricing.fetch_key_rates(key_from, today).await.ok();
    let inflation_from = today
        .checked_sub_months(Months::new(INFLATION_MONTHS_BACK))
        .unwrap_or(today);
    let mut inflation = Vec::new();
    for source in InflationSource::ALL {
        if let Ok(readings) = pricing.fetch_inflation(source, inflation_from, today).await {
            inflation.extend(readings);
        }
    }
    FetchedRates {
        key_rates,
        inflation,
    }
}

/// Fetch, apply, tell the user about new figures and emit `rates-updated`.
/// Fails with `NETWORK` only when no source answered at all.
pub async fn refresh_rates_now(app: &AppHandle) -> Result<Vec<InflationCurrency>, String> {
    let state = app.state::<AppState>();
    // Locked: nothing to apply to, so do not fetch either.
    drop(require_vault(&state).map_err(err_to_string)?);
    let today = chrono::Utc::now().date_naive();
    let fetched = fetch_rates(&state.pricing, today).await;
    if fetched.key_rates.is_none() && fetched.inflation.is_empty() {
        return Err(err_to_string(trade_core::Error::Network(
            "no rate source answered".into(),
        )));
    }
    let russian = crate::scheduler::russian(app);
    let manual = current_settings(app).inflation;
    let (fresh, overview) = {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard.as_mut().unwrap();
        let notices = vault
            .apply_rates(&fetched, russian)
            .map_err(err_to_string)?;
        let fresh = vault.log_notices(notices).map_err(err_to_string)?;
        (fresh, vault.inflation_overview(&manual))
    };
    crate::scheduler::deliver(app, fresh, false);
    let _ = app.emit("rates-updated", ());
    Ok(overview)
}

/// Refresh inflation and the key rate now.
#[tauri::command]
pub async fn refresh_rates(app: AppHandle) -> Result<Vec<InflationCurrency>, String> {
    refresh_rates_now(&app).await
}

/// Each currency's inflation: mode, source, latest month and series.
#[tauri::command]
pub async fn get_inflation_overview(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<InflationCurrency>, String> {
    let manual = current_settings(&app).inflation;
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().inflation_overview(&manual))
}

/// Pick Auto / Manual for a currency (`null` = back to the default).
#[tauri::command]
pub async fn set_inflation_mode(
    currency: String,
    mode: Option<InflationMode>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<InflationCurrency>, String> {
    let manual = current_settings(&app).inflation;
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    vault
        .set_inflation_mode(&currency, mode)
        .map_err(err_to_string)?;
    let overview = vault.inflation_overview(&manual);
    drop(guard);
    let _ = app.emit("rates-updated", ());
    Ok(overview)
}
