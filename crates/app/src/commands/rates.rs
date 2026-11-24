//! Inflation and the key rate from the statistics offices: the background
//! refresh, the "Refresh" button and the per-currency mode.

use crate::ipc::events::{AppEvent, RatesUpdated};
use chrono::{Datelike, Months, NaiveDate};
use mwt_model::{InflationMode, InflationSource};
use mwt_money::inflation as inflation_reads;
use mwt_money::inflation::{FetchedRates, InflationCurrency};
use mwt_net::PricingService;
use mwt_usecases::usecases::money::inflation;
use mwt_usecases::usecases::planning::notifications;
use tauri::{AppHandle, Manager, State};

use crate::commands::{AppState, require_vault};
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

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
pub async fn refresh_rates_now(app: &AppHandle) -> Result<Vec<InflationCurrency>, IpcError> {
    let state = app.state::<AppState>();
    // Locked: nothing to apply to, so do not fetch either.
    drop(require_vault(&state)?);
    let today = chrono::Utc::now().date_naive();
    let fetched = fetch_rates(&state.pricing, today).await;
    if fetched.key_rates.is_none() && fetched.inflation.is_empty() {
        return Err(IpcError::from(mwt_base::Error::Network(
            "no rate source answered".into(),
        )));
    }
    let russian = crate::scheduler::russian(app);
    let (fresh, overview) = {
        let mut guard = require_vault(&state)?;
        let vault = guard.as_mut().unwrap();
        let notices = inflation::apply_rates(vault, &fetched, russian)?;
        let fresh = notifications::log_notices(vault, notices)?;
        (fresh, inflation_reads::inflation_overview(&vault.doc))
    };
    crate::scheduler::deliver(app, fresh, false);
    let _ = RatesUpdated::emit(app, &());
    Ok(overview)
}

/// Refresh inflation and the key rate now.
#[tauri::command]
pub async fn refresh_rates(app: AppHandle) -> Result<Vec<InflationCurrency>, IpcError> {
    refresh_rates_now(&app).await
}

/// Each currency's inflation: mode, source, latest month and series.
#[tauri::command]
pub async fn get_inflation_overview(
    state: State<'_, AppState>,
) -> Result<Vec<InflationCurrency>, IpcError> {
    with_vault(&state, |vault| {
        Ok(inflation_reads::inflation_overview(&vault.doc))
    })
}

/// Pick Auto / Manual for a currency (`null` = back to the default).
#[tauri::command]
pub async fn set_inflation_mode(
    currency: String,
    mode: Option<InflationMode>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<InflationCurrency>, IpcError> {
    let overview = with_vault_mut::<_, IpcError>(&state, |vault| {
        inflation::set_inflation_mode(vault, &currency, mode)?;
        Ok(inflation_reads::inflation_overview(&vault.doc))
    })?;
    let _ = RatesUpdated::emit(&app, &());
    Ok(overview)
}

ipc_commands!(rates_commands, RATES_NAMES, commands::rates: [refresh_rates, get_inflation_overview, set_inflation_mode]);
