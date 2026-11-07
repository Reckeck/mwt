use trade_core::error::Error;
use trade_core::ipc_contract::VaultInfo;
use trade_core::ipc_contract::security::{
    BiometricEnableInput, BiometricState, BiometricUnlockInput,
};

use crate::commands::err_to_string;

#[tauri::command]
pub async fn biometric_enable(input: BiometricEnableInput) -> Result<BiometricState, String> {
    let _ = input;
    Err(err_to_string(Error::Unsupported))
}

#[tauri::command]
pub async fn biometric_disable() -> Result<BiometricState, String> {
    Err(err_to_string(Error::Unsupported))
}

#[tauri::command]
pub async fn biometric_unlock(input: BiometricUnlockInput) -> Result<VaultInfo, String> {
    let _ = input;
    Err(err_to_string(Error::Unsupported))
}
