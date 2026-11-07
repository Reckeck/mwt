use trade_core::error::Error;
use trade_core::ipc_contract::security::AutolockSettings;

use crate::commands::err_to_string;

#[tauri::command]
pub async fn set_autolock(input: AutolockSettings) -> Result<AutolockSettings, String> {
    let _ = input;
    Err(err_to_string(Error::Unsupported))
}

#[tauri::command]
pub async fn autolock_activity() -> Result<(), String> {
    Err(err_to_string(Error::Unsupported))
}
