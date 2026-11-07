use trade_core::error::Error;
use trade_core::ipc_contract::security::{
    PasswordStrengthReport, RaiseToCryptoInput, RaiseToCryptoReport, ValidatePasswordStrengthInput,
};

use crate::commands::err_to_string;

#[tauri::command]
pub async fn raise_to_crypto_level(
    input: RaiseToCryptoInput,
) -> Result<RaiseToCryptoReport, String> {
    let _ = input;
    Err(err_to_string(Error::Unsupported))
}

#[tauri::command]
pub async fn validate_password_strength(
    input: ValidatePasswordStrengthInput,
) -> Result<PasswordStrengthReport, String> {
    let _ = input;
    Err(err_to_string(Error::Unsupported))
}
