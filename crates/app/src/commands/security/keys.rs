//! Where an exchange key stands on this device (H2).

use tauri::State;
use trade_core::ipc_contract::ExchangeKeyStatusInput;
use trade_core::ipc_contract::security::ExchangeKeyState;

use crate::commands::{AppState, err_to_string, require_vault};

/// `missing` / `not_verified` / `read_only` / `level_required` for the key
/// serving a store. The hint and sub-accounts stay in `exchange_key_status`.
/// Another device's key is not here: a device without its own key reads
/// `missing`.
#[tauri::command]
pub async fn read_exchange_key_state(
    input: ExchangeKeyStatusInput,
    state: State<'_, AppState>,
) -> Result<ExchangeKeyState, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().exchange_key_state(&input.store_id))
}
