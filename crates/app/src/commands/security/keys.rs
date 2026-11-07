use trade_core::error::Error;
use trade_core::ipc_contract::ExchangeKeyStatusInput;
use trade_core::ipc_contract::security::ExchangeKeyState;

use crate::commands::err_to_string;

#[tauri::command]
pub async fn read_exchange_key_state(
    input: ExchangeKeyStatusInput,
) -> Result<ExchangeKeyState, String> {
    let _ = input;
    Err(err_to_string(Error::Unsupported))
}
