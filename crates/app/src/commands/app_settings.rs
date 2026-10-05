//! App-level settings — theme, language, base currency and favourite
//! currencies.
//!
//! These describe the *user*, not a vault, so they live in the app config
//! directory as plain JSON (`app_settings.json`) instead of being encrypted into
//! every vault. Opening a second vault therefore keeps the look and the base
//! currency the user already chose, and the settings work before any vault is
//! unlocked.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use trade_core::appconfig::{
    AppSettings, UpdateAppSettingsInput, build_currency_catalog, load_settings, save_settings,
};
use trade_core::ipc_contract::CurrencyCatalogOutput;

use crate::commands::{AppState, err_to_string, lock_vault};

/// App config directory. Panics only if the platform refuses to name it, which
/// would leave the app with nowhere to keep either this or the recent vaults.
fn config_dir(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().expect("app config dir")
}

/// Read the app settings. Never fails: a missing or corrupt file yields the
/// defaults, and the lock screen must be able to render them.
#[tauri::command]
pub async fn get_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    Ok(load_settings(&config_dir(&app)))
}

/// Persist a partial update and report the resulting settings.
///
/// Changing the base currency also re-derives the open vault's holdings and
/// average costs, so the numbers on screen match the currency the UI now shows.
#[tauri::command]
pub async fn update_app_settings(
    app: AppHandle,
    input: UpdateAppSettingsInput,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    let dir = config_dir(&app);
    let mut settings = load_settings(&dir);
    settings.apply(&input);
    save_settings(&dir, &settings).map_err(err_to_string)?;

    // Push the base currency into the vault if one is open. The guard is taken
    // and released without any `.await` in between.
    let mut guard = lock_vault(&state).map_err(err_to_string)?;
    if let Some(vault) = guard.as_mut() {
        vault.set_base_currency(settings.base());
    }
    drop(guard);

    Ok(settings)
}

/// Currencies the base/favourite pickers may offer.
///
/// Goes to the network first, then to the last successfully fetched catalog,
/// then to a built-in list — see [`build_currency_catalog`]. The `notice` says
/// when the built-in list was used so the UI can flag it as possibly
/// incomplete.
#[tauri::command]
pub async fn get_supported_currencies(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CurrencyCatalogOutput, String> {
    let dir = config_dir(&app);
    let cached = trade_core::appconfig::load_currencies(&dir);
    let catalog = build_currency_catalog(&dir, &state.pricing).await;
    let notice = match &cached {
        // Nothing was ever fetched, so this is the built-in list.
        None => Some("builtin"),
        Some(previous) if *previous == catalog => Some("cached"),
        _ => None,
    };
    Ok(CurrencyCatalogOutput {
        catalog,
        notice: notice.map(str::to_string),
    })
}

/// Apply the stored base currency to the vault that is currently open.
///
/// The vault cannot know it — the value lives outside the document — so the app
/// layer pushes it in right after `open`/`create`. Without this an unlocked
/// vault would report figures in USD while the UI labels them with the user's
/// base currency.
pub fn sync_base_currency(state: &AppState, app: &AppHandle) {
    let settings = load_settings(&config_dir(app));
    // A poisoned lock must not fail the unlock that just succeeded.
    if let Ok(mut guard) = lock_vault(state)
        && let Some(vault) = guard.as_mut()
    {
        vault.set_base_currency(settings.base());
    }
}
