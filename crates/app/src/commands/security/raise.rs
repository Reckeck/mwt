use crate::ipc::events::{AppEvent, SecurityRaiseProgress};
use tauri::{AppHandle, State};
use trade_core::error::Error;
use trade_core::ipc_contract::security::{
    PasswordStrengthReport, RaiseProgress, RaiseToCryptoInput, RaiseToCryptoReport, SecurityLevel,
    ValidatePasswordStrengthInput,
};
use trade_core::service::security_level::password_strength;

use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// Move the open vault to level 3 (`UnlockedVault::raise_to_crypto`): backup,
/// re-encryption at the hardened cost of this device (of a phone if the vault
/// has an external copy), check, copies. The backup goes next to the vault;
/// another place is not supported yet (`backup_path` must be empty).
#[tauri::command]
pub async fn raise_to_crypto_level(
    input: RaiseToCryptoInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RaiseToCryptoReport, IpcError> {
    crate::commands::sandbox::refuse(&state, "the crypto level")?;
    if !input.backup_path.is_empty() {
        return Err(IpcError::from(Error::InvalidInput(
            "the backup goes next to the vault".into(),
        )));
    }
    let profile = {
        let guard = require_vault(&state)?;
        guard.as_ref().ok_or("Vault is locked")?.raise_profile()
    };
    // Measured without the vault lock: it takes a few derivations.
    let kdf = profile.calibrate_here();
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().ok_or("Vault is locked")?;
    let report =
        vault.raise_to_crypto(&input.current_password, &input.new_password, kdf, |stage| {
            let _ = SecurityRaiseProgress::emit(&app, &RaiseProgress { stage });
        })?;
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
) -> Result<PasswordStrengthReport, IpcError> {
    Ok(password_strength(&input.password))
}

ipc_commands!(security_raise_commands, SECURITY_RAISE_NAMES, commands::security::raise: [raise_to_crypto_level, validate_password_strength]);
