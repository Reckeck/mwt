use tauri::State;

use trade_core::ipc_contract::UpdateSettingsInput;
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
    let out = s.clone();
    v.persist().map_err(crate::commands::err_to_string)?;
    Ok(out)
}
