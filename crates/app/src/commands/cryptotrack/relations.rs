//! Who an account dealt with: the user's other addresses and exchanges, the
//! protocols met, and the fiat operations paired with its operations.
//!
//! Owner: spec E15, with `service/cryptotrack/relations.rs`. Computed from the
//! events on every call and never stored.

use tauri::State;

use trade_core::ipc_contract::cryptotrack::relations::{CryptoRelations, CryptoRelationsInput};

use crate::commands::AppState;
use crate::commands::planning::with_vault;
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use trade_core::usecases::cryptotrack::relations;

/// The relations of one account, each row carrying the operation filter it
/// opens. Read only.
#[tauri::command]
pub async fn crypto_relations(
    input: CryptoRelationsInput,
    state: State<'_, AppState>,
) -> Result<CryptoRelations, IpcError> {
    with_vault(&state, |vault| {
        relations::crypto_relations(&vault.books(), &input)
    })
}

ipc_commands!(cryptotrack_relations_commands, CRYPTOTRACK_RELATIONS_NAMES, commands::cryptotrack::relations: [crypto_relations]);
