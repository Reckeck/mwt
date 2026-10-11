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

/// The IANA zone days and months are cut in (plan §7.6): the one chosen in
/// the settings, else the OS zone, else UTC.
pub fn current_zone(app: &AppHandle) -> String {
    current_settings(app).zone_or(iana_time_zone::get_timezone().ok().as_deref())
}

/// The app settings as stored now (defaults when the file is missing).
pub fn current_settings(app: &AppHandle) -> AppSettings {
    load_settings(&config_dir(app))
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

/// Can the app own a system-wide key here, and what should a compositor
/// binding run otherwise.
#[tauri::command]
pub async fn get_shortcut_support() -> Result<trade_core::ipc_contract::ShortcutSupport, String> {
    let exe = std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "trade-app".into());
    Ok(trade_core::ipc_contract::ShortcutSupport {
        global_keys: crate::shortcut::global_keys_supported(),
        launch_command: format!("{exe} {}", crate::shortcut::QUICK_ADD_FLAG),
    })
}

/// Set or clear the system-wide «+ Операция» key. The combination is
/// registered first and saved only if that worked, so the settings never
/// claim a shortcut the system refused (`SHORTCUT_FAILED`).
#[tauri::command]
pub async fn set_quick_add_shortcut(
    app: AppHandle,
    shortcut: Option<String>,
) -> Result<AppSettings, String> {
    let dir = config_dir(&app);
    let mut settings = load_settings(&dir);
    let wanted = shortcut
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty());
    if let Err(err) = crate::shortcut::apply(&app, wanted.as_deref()) {
        // Put the previous one back; if that fails too, nothing is registered.
        crate::shortcut::apply(&app, settings.quick_add_shortcut.as_deref()).ok();
        return Err(err);
    }
    settings.quick_add_shortcut = wanted;
    save_settings(&dir, &settings).map_err(err_to_string)?;
    Ok(settings)
}

/// Currencies the base/favourite pickers may offer.
///
/// Serves a cached catalog younger than a day as is; otherwise goes to the
/// network first, then to the last successfully fetched catalog,
/// then to a built-in list — see [`build_currency_catalog`]. The `notice` is set
/// only when no provider answered, so the UI can flag the list as possibly
/// stale or incomplete.
#[tauri::command]
pub async fn get_supported_currencies(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CurrencyCatalogOutput, String> {
    let dir = config_dir(&app);
    let (catalog, source) = build_currency_catalog(&dir, &state.pricing).await;
    Ok(CurrencyCatalogOutput {
        catalog,
        notice: source.notice().map(str::to_string),
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
