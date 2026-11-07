use trade_core::error::Error;
use trade_core::ipc_contract::security::SecurityState;

use crate::commands::err_to_string;

#[tauri::command]
pub async fn security_state() -> Result<SecurityState, String> {
    Err(err_to_string(Error::Unsupported))
}
