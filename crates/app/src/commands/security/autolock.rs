use mwt_usecases::appconfig::validate_autolock;
use mwt_usecases::ipc_contract::security::AutolockSettings;
use tauri::AppHandle;

use crate::commands::app_settings;
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// Save this device's autolock. The range is 5-60 minutes; the lock can only
/// be turned off while the device holds no exchange keys.
#[tauri::command]
pub async fn set_autolock(
    input: AutolockSettings,
    app: AppHandle,
) -> Result<AutolockSettings, IpcError> {
    let checked = validate_autolock(&input, crate::autolock::has_device_keys(&app))?;
    let dir = app_settings::config_dir(&app);
    let mut settings = app_settings::current_settings(&app);
    settings.autolock = Some(checked.clone());
    app_settings::save(&dir, &settings)?;
    // Turned on again: the idle time starts now, not at the last touch, and
    // on Android the background watch has to be polling.
    crate::autolock::touch();
    #[cfg(target_os = "android")]
    crate::sync::background::ensure_polling(&app);
    Ok(checked)
}

/// The window reports the user is active (a touch, a key); the frontend sends
/// it at most once per 30 s, the timer itself runs here.
#[tauri::command]
pub async fn autolock_activity() -> Result<(), IpcError> {
    crate::autolock::touch();
    Ok(())
}

/// A system picker or the camera is about to open from a plain webview input;
/// it takes the app to the background without the user leaving. The hold is
/// time-limited and ends when the app is back in the foreground.
#[tauri::command]
pub async fn autolock_hold_for_picker() -> Result<(), IpcError> {
    crate::autolock::hold_for_system_picker();
    Ok(())
}

ipc_commands!(security_autolock_commands, SECURITY_AUTOLOCK_NAMES, commands::security::autolock: [set_autolock, autolock_activity, autolock_hold_for_picker]);
