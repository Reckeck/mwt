use tauri::{AppHandle, Emitter, State};
use trade_core::error::Error;
use trade_core::ipc_contract::security::{
    PasswordStrengthReport, RaiseProgress, RaiseToCryptoInput, RaiseToCryptoReport, SecurityLevel,
    ValidatePasswordStrengthInput,
};
use trade_core::service::security_level::password_strength;

use crate::commands::{AppState, err_to_string, require_vault};

/// Event with the stage the raise is about to start.
pub const RAISE_PROGRESS_EVENT: &str = "security-raise-progress";

/// Move the open vault to level 3 (`UnlockedVault::raise_to_crypto`): backup,
/// re-encryption at the hardened cost of this device (of a phone if the vault
/// has an external copy), check, copies. The backup goes next to the vault;
/// another place is not supported yet (`backup_path` must be empty).
#[tauri::command]
pub async fn raise_to_crypto_level(
    input: RaiseToCryptoInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RaiseToCryptoReport, String> {
    crate::commands::sandbox::refuse(&state, "the crypto level")?;
    if !input.backup_path.is_empty() {
        return Err(err_to_string(Error::InvalidInput(
            "the backup goes next to the vault".into(),
        )));
    }
    let profile = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        guard.as_ref().ok_or("Vault is locked")?.raise_profile()
    };
    // Measured without the vault lock: it takes a few derivations.
    let kdf = profile.calibrate_here();
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().ok_or("Vault is locked")?;
    let report = vault
        .raise_to_crypto(&input.current_password, &input.new_password, kdf, |stage| {
            let _ = app.emit(RAISE_PROGRESS_EVENT, RaiseProgress { stage });
        })
        .map_err(err_to_string)?;
    Ok(RaiseToCryptoReport {
        level: SecurityLevel::Crypto,
        backup_path: report.backup.display().to_string(),
    })
}

/// The strength of a candidate level-3 password, by the same rule the raise
/// applies. The password is only graded, never kept.
#[tauri::command]
pub async fn validate_password_strength(
    input: ValidatePasswordStrengthInput,
) -> Result<PasswordStrengthReport, String> {
    Ok(password_strength(&input.password))
}
