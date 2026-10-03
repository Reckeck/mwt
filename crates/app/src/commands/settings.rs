use tauri::State;

use trade_core::ipc_contract::{DevDumpVaultOutput, UpdateSettingsInput};
use trade_core::model::vault::Settings;

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    Ok(g.as_ref().unwrap().doc.settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    input: UpdateSettingsInput,
    state: State<'_, AppState>,
) -> Result<Settings, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let s = &mut v.doc.settings;
    if let Some(base) = input.base_currency
        && !base.trim().is_empty()
    {
        s.base_currency = base.trim().to_lowercase();
    }
    if let Some(t) = input.theme {
        s.theme = t;
    }
    if let Some(l) = input.locale {
        s.locale = l;
    }
    if let Some(d) = input.display_decimals {
        s.display_decimals = d.clamp(0, 18);
    }
    if let Some(fc) = input.favorite_currencies {
        s.favorite_currencies = fc
            .into_iter()
            .map(|x| x.trim().to_uppercase())
            .filter(|x| !x.is_empty() && x.len() == 3)
            .collect::<Vec<_>>();
        s.favorite_currencies.dedup();
    }
    if let Some(dw) = input.dashboard_widgets {
        s.dashboard_widgets = dw;
    }
    if let Some(fx) = input.fx_refresh_interval_sec {
        s.fx_refresh_interval_sec = fx.clamp(30, 3600);
    }
    if let Some(m) = input.minimize_to_tray {
        s.minimize_to_tray = m;
    }
    if let Some(b) = input.bybit_import_interval_sec {
        // 0 disables the poller; otherwise keep a sane floor so a typo cannot
        // hammer the Bybit API every few seconds.
        s.bybit_import_interval_sec = if b == 0 { 0 } else { b.clamp(60, 86_400) };
    }
    let out = s.clone();
    v.persist().map_err(crate::commands::err_to_string)?;
    Ok(out)
}

#[tauri::command]
pub async fn dev_dump_vault(state: State<'_, AppState>) -> Result<DevDumpVaultOutput, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_ref().unwrap();
    let json =
        serde_json::to_string_pretty(&v.doc).map_err(|e| format!("SERIALIZE_ERROR: {}", e))?;
    Ok(DevDumpVaultOutput { json })
}
