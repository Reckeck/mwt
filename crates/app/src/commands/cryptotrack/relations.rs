//! Who an account dealt with: the user's other addresses and exchanges, the
//! protocols met, and the fiat operations paired with its operations.
//!
//! Owner: spec E15, with `service/cryptotrack/relations.rs`. Computed from the
//! events on every call and never stored.

use tauri::State;

use trade_core::ipc_contract::cryptotrack::relations::{CryptoRelations, CryptoRelationsInput};

use crate::commands::AppState;
use crate::commands::planning::with_vault;

/// The relations of one account, each row carrying the operation filter it
/// opens. Read only.
#[tauri::command]
pub async fn crypto_relations(
    input: CryptoRelationsInput,
    state: State<'_, AppState>,
) -> Result<CryptoRelations, String> {
    with_vault(&state, |vault| vault.crypto_relations(&input))
}
