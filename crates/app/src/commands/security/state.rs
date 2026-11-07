use tauri::{AppHandle, State};
use trade_core::ipc_contract::security::{SecurityLevel, SecurityState};
use trade_core::service::security_level::level_of_vault_file;

use crate::commands::{AppState, app_settings, err_to_string, lock_vault};

/// The protection of the open vault, or, while it is locked, of the vault
/// file the lock screen shows (then the lowest level is what the file has:
/// the document that would tell more is not readable yet).
#[tauri::command]
pub async fn security_state(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SecurityState, String> {
    let open = lock_vault(&state)
        .map_err(err_to_string)?
        .as_ref()
        .map(|vault| {
            (
                vault.path.clone(),
                vault.security_level(),
                vault.min_allowed_level(),
            )
        });
    let (path, level, min_allowed_level) = match open {
        Some(found) => found,
        None => {
            let path = state
                .vault_path
                .lock()
                .map_err(|_| "lock".to_string())?
                .clone();
            let level = level_of_vault_file(&path).unwrap_or(SecurityLevel::Ledger);
            (path, level, level)
        }
    };
    let settings = app_settings::current_settings(&app);
    let wait = settings.pin_retry_after(&path.to_string_lossy(), crate::commands::auth::now_secs());
    Ok(SecurityState {
        level,
        min_allowed_level,
        has_device_keys: crate::autolock::has_device_keys(&app),
        autolock: settings.autolock(),
        biometric: super::biometric::state_of(&app, &path),
        pin_retry_after_secs: (level == SecurityLevel::Pin && wait > 0)
            .then(|| u32::try_from(wait).unwrap_or(u32::MAX)),
    })
}
