use tauri::State;

use trade_core::ipc_contract::{DevDumpVaultOutput, UpdateSettingsInput};
use trade_core::model::vault::Settings;

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().doc.settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    input: UpdateSettingsInput,
    state: State<'_, AppState>,
) -> Result<Settings, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    let settings = &mut vault.doc.settings;
    if let Some(base) = input.base_currency
        && !base.trim().is_empty()
    {
        settings.base_currency = base.trim().to_lowercase();
    }
    if let Some(theme) = input.theme {
        settings.theme = theme;
    }
    if let Some(locale) = input.locale {
        settings.locale = locale;
    }
    if let Some(decimals) = input.display_decimals {
        settings.display_decimals = decimals.clamp(0, 18);
    }
    if let Some(favorites) = input.favorite_currencies {
        settings.favorite_currencies = favorites
            .into_iter()
            .map(|code| code.trim().to_uppercase())
            .filter(|code| !code.is_empty() && code.len() == 3)
            .collect::<Vec<_>>();
        settings.favorite_currencies.dedup();
    }
    if let Some(widgets) = input.dashboard_widgets {
        settings.dashboard_widgets = widgets;
    }
    if let Some(interval) = input.fx_refresh_interval_sec {
        settings.fx_refresh_interval_sec = interval.clamp(30, 3600);
    }
    if let Some(minimize) = input.minimize_to_tray {
        settings.minimize_to_tray = minimize;
    }
    if let Some(interval) = input.bybit_import_interval_sec {
        // 0 disables the poller; otherwise keep a sane floor so a typo cannot
        // hammer the Bybit API every few seconds.
        settings.bybit_import_interval_sec = if interval == 0 {
            0
        } else {
            interval.clamp(60, 86_400)
        };
    }
    let out = settings.clone();
    vault.persist().map_err(crate::commands::err_to_string)?;
    Ok(out)
}

#[tauri::command]
pub async fn dev_dump_vault(state: State<'_, AppState>) -> Result<DevDumpVaultOutput, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    let json = serde_json::to_string_pretty(&vault.doc)
        .map_err(|err| format!("SERIALIZE_ERROR: {}", err))?;
    Ok(DevDumpVaultOutput { json })
}
