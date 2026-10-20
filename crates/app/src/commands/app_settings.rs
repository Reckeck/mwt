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
    AppSettings, UpdateAppSettingsInput, build_currency_catalog, load_or_init_settings,
    save_settings,
};
use trade_core::ipc_contract::CurrencyCatalogOutput;

use crate::commands::{AppState, err_to_string, lock_vault};

/// App config directory. Panics only if the platform refuses to name it, which
/// would leave the app with nowhere to keep either this or the recent vaults.
pub fn config_dir(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().expect("app config dir")
}

/// The IANA zone days and months are cut in (plan §7.6): the one chosen in
/// the settings, else the OS zone, else UTC.
pub fn current_zone(app: &AppHandle) -> String {
    current_settings(app).zone_or(iana_time_zone::get_timezone().ok().as_deref())
}

type Seen = (std::path::PathBuf, std::time::SystemTime, u64, AppSettings);
static CACHE: std::sync::Mutex<Option<Seen>> = std::sync::Mutex::new(None);

/// Write the app settings and drop the cached copy (a same-size rewrite within
/// the file system's time resolution would otherwise look unchanged).
pub fn save(dir: &std::path::Path, settings: &AppSettings) -> trade_core::Result<()> {
    let result = save_settings(dir, settings);
    if let Ok(mut cache) = CACHE.lock() {
        *cache = None;
    }
    result
}

/// The settings file, stamped with today's date on the very first run.
///
/// Read on every command that needs the zone or the locale, by the scheduler
/// and by the phone's sync poll: the parsed file is kept and re-read only when
/// its size or time changed (any writer, anywhere, invalidates it that way).
fn load(dir: &std::path::Path) -> AppSettings {
    let path = trade_core::appconfig::settings_path(dir);
    let stamp = std::fs::metadata(&path)
        .ok()
        .and_then(|meta| Some((meta.modified().ok()?, meta.len())));
    if let Some((modified, len)) = stamp
        && let Ok(cache) = CACHE.lock()
        && let Some((seen_path, seen_time, seen_len, settings)) = cache.as_ref()
        && *seen_path == path
        && *seen_time == modified
        && *seen_len == len
    {
        return settings.clone();
    }
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let settings = load_or_init_settings(dir, &today);
    // Cached under the stamp seen *before* reading: a write that lands while this
    // read runs gives the file a new stamp, so the next call reads it again. Taken
    // after the read, the old content would sit under the new stamp for good.
    if let Some((modified, len)) = stamp
        && let Ok(mut cache) = CACHE.lock()
    {
        *cache = Some((path, modified, len, settings.clone()));
    }
    settings
}

/// The app settings as stored now (defaults when the file is missing).
pub fn current_settings(app: &AppHandle) -> AppSettings {
    load(&config_dir(app))
}

/// The OS the app runs on (`linux`, `windows`, `macos`, `android`, `ios`):
/// the settings show tray and autostart only where they exist.
#[tauri::command]
pub fn app_platform() -> &'static str {
    std::env::consts::OS
}

/// Read the app settings. Never fails: a missing or corrupt file yields the
/// defaults, and the lock screen must be able to render them.
#[tauri::command]
pub async fn get_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    Ok(load(&config_dir(&app)))
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
    let mut settings = load(&dir);
    settings.apply(&input);
    save(&dir, &settings).map_err(err_to_string)?;

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
        .unwrap_or_else(|_| "multi-wallet-tracker".into());
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
    let mut settings = load(&dir);
    let wanted = shortcut
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty());
    if let Err(err) = crate::shortcut::apply(&app, wanted.as_deref()) {
        // Put the previous one back; if that fails too, nothing is registered.
        crate::shortcut::apply(&app, settings.quick_add_shortcut.as_deref()).ok();
        return Err(err);
    }
    settings.quick_add_shortcut = wanted;
    save(&dir, &settings).map_err(err_to_string)?;
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
    let dir = config_dir(app);
    let mut settings = load(&dir);
    // A poisoned lock must not fail the unlock that just succeeded.
    if let Ok(mut guard) = lock_vault(state)
        && let Some(vault) = guard.as_mut()
    {
        vault.set_base_currency(settings.base());
        // The tray, refresh interval and decimals used to be kept per vault;
        // the first vault opened after the update hands them to this device.
        let adopted = settings.adopt_device_settings(&vault.doc.settings);
        let identified = settings.ensure_device_id();
        drop(guard);
        if adopted || identified {
            save(&dir, &settings).ok();
        }
    }
}

/// Give this device its friendly name (an adjective and a fruit, in the
/// app's language) if it has none yet.
pub fn ensure_device_name(app: &AppHandle) {
    let dir = config_dir(app);
    let mut settings = load(&dir);
    if settings.ensure_device_name(crate::scheduler::russian(app)) {
        save(&dir, &settings).ok();
    }
}

/// This installation's sync id, made and saved on first use.
pub fn device_id(app: &AppHandle) -> String {
    let dir = config_dir(app);
    let mut settings = load(&dir);
    if settings.ensure_device_id() {
        save(&dir, &settings).ok();
    }
    settings.device_id
}
