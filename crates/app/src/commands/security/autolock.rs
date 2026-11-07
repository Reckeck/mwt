use tauri::AppHandle;
use trade_core::appconfig::validate_autolock;
use trade_core::ipc_contract::security::AutolockSettings;

use crate::commands::{app_settings, err_to_string};

/// Save this device's autolock. The range is 5-60 minutes; the lock can only
/// be turned off while the device holds no exchange keys.
#[tauri::command]
pub async fn set_autolock(
    input: AutolockSettings,
    app: AppHandle,
) -> Result<AutolockSettings, String> {
    let checked =
        validate_autolock(&input, crate::autolock::has_device_keys(&app)).map_err(err_to_string)?;
    let dir = app_settings::config_dir(&app);
    let mut settings = app_settings::current_settings(&app);
    settings.autolock = Some(checked.clone());
    app_settings::save(&dir, &settings).map_err(err_to_string)?;
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
pub async fn autolock_activity() -> Result<(), String> {
    crate::autolock::touch();
    Ok(())
}

/// A system picker or the camera is about to open from a plain webview input;
/// it takes the app to the background without the user leaving. The hold is
/// time-limited and ends when the app is back in the foreground.
#[tauri::command]
pub async fn autolock_hold_for_picker() -> Result<(), String> {
    crate::autolock::hold_for_system_picker();
    Ok(())
}
