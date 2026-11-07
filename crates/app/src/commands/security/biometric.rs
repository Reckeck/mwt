//! Quick unlock of a level-3 vault (H7A). The password is bound to the device by
//! the OS (`crate::biometric`); here are the commands, the level check and the
//! hand-over to the ordinary unlock. Errors go out as `"BIOMETRIC_*: message"`.

use std::path::Path;

use tauri::{AppHandle, State};
use trade_core::ipc_contract::security::{
    BiometricEnableInput, BiometricState, BiometricUnlockInput, SecurityLevel,
};
use trade_core::ipc_contract::{UnlockInput, VaultInfo};
use trade_core::service::security_level::level_of_vault_file;

use crate::biometric::{BioError, system_gate};
use crate::commands::{AppState, app_settings, err_to_string, require_vault};

/// Quick unlock exists only for level 3; below it the state is `Unavailable`.
pub(crate) fn state_of(app: &AppHandle, path: &Path) -> BiometricState {
    let level_three = level_of_vault_file(path).is_ok_and(|level| level == SecurityLevel::Crypto);
    if !level_three {
        return BiometricState::Unavailable;
    }
    system_gate(&app_settings::config_dir(app)).state(&path.to_string_lossy())
}

/// The state for the vault file the lock screen shows (it is not open yet).
#[tauri::command]
pub async fn biometric_state(
    input: BiometricUnlockInput,
    app: AppHandle,
) -> Result<BiometricState, String> {
    let path =
        crate::platform_fs::resolve_vault_to_open(&app, &input.path).map_err(err_to_string)?;
    Ok(state_of(&app, &path))
}

/// Bind the password of the open vault to the device. The password is checked
/// here first, so a typo cannot become the quick-unlock secret.
#[tauri::command]
pub async fn biometric_enable(
    input: BiometricEnableInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BiometricState, String> {
    let path = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard.as_ref().ok_or("Vault is locked")?;
        if vault.security_level() != SecurityLevel::Crypto {
            return Err(BioError::Unavailable.to_ipc());
        }
        vault
            .check_password(&input.password)
            .map_err(err_to_string)?;
        vault.path.clone()
    };
    let dir = app_settings::config_dir(&app);
    let id = path.to_string_lossy().into_owned();
    let secret = zeroize::Zeroizing::new(input.password.into_bytes());
    tokio::task::spawn_blocking(move || system_gate(&dir).enable(&id, &secret))
        .await
        .map_err(|_| BioError::Failed.to_ipc())?
        .map_err(BioError::to_ipc)?;
    Ok(state_of(&app, &path))
}

#[tauri::command]
pub async fn biometric_disable(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BiometricState, String> {
    let path = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        guard.as_ref().ok_or("Vault is locked")?.path.clone()
    };
    system_gate(&app_settings::config_dir(&app)).disable(&path.to_string_lossy());
    Ok(state_of(&app, &path))
}

/// Release the password after the OS gesture and open the vault with it. This is
/// not a typed password: the 7-day window keeps running.
#[tauri::command]
pub async fn biometric_unlock(
    input: BiometricUnlockInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let path =
        crate::platform_fs::resolve_vault_to_open(&app, &input.path).map_err(err_to_string)?;
    if state_of(&app, &path) == BiometricState::Unavailable {
        return Err(BioError::Unavailable.to_ipc());
    }
    let dir = app_settings::config_dir(&app);
    let id = path.to_string_lossy().into_owned();
    let secret = tokio::task::spawn_blocking(move || system_gate(&dir).unlock(&id))
        .await
        .map_err(|_| BioError::Failed.to_ipc())?
        .map_err(BioError::to_ipc)?;
    let password = String::from_utf8(secret.to_vec()).map_err(|_| BioError::Failed.to_ipc())?;
    let opened = crate::commands::auth::unlock_with(
        UnlockInput {
            path: input.path,
            password,
        },
        &app,
        &state,
        false,
    )
    .await;
    if let Err(message) = &opened
        && message.starts_with("SECRET")
    {
        // The stored password no longer opens the vault: the binding is stale.
        system_gate(&app_settings::config_dir(&app)).disable(&path.to_string_lossy());
        return Err(BioError::Invalidated.to_ipc());
    }
    opened
}
