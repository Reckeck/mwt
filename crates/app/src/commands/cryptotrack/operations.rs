//! The module's operations: the filtered list, one operation's detail and the
//! corrections over what a source reported.
//!
//! Owner: spec E10, with `service/cryptotrack/operations.rs`. There is no
//! command that creates or deletes an operation, on this screen or any other:
//! events come from the sources, a correction only reclassifies.
//!
//! Nothing here reaches the network. The detail names the page of an
//! operation (`https` only); the frontend opens it with `open_explorer` on
//! the user's click, and not before.

use tauri::State;

use trade_core::ipc_contract::cryptotrack::operations::{
    CryptoListOperationsInput, CryptoOperationDetail, CryptoOperationDetailInput, CryptoOpsPage,
    CryptoSetOverrideInput, CryptoSetOverrideResult,
};

use crate::commands::AppState;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use trade_core::usecases::cryptotrack::operations;

/// The operations of the followed accounts, one page of the filtered list.
#[tauri::command]
pub async fn crypto_list_operations(
    input: CryptoListOperationsInput,
    state: State<'_, AppState>,
) -> Result<CryptoOpsPage, IpcError> {
    with_vault(&state, |vault| {
        Ok(operations::crypto_list_operations(&vault.books(), &input))
    })
}

/// One operation with its legs, its gas, its chain and what the source
/// answered.
#[tauri::command]
pub async fn crypto_operation_detail(
    input: CryptoOperationDetailInput,
    state: State<'_, AppState>,
) -> Result<CryptoOperationDetail, IpcError> {
    with_vault(&state, |vault| {
        operations::crypto_operation_detail(&vault.books(), &input)
    })
}

/// Put a correction over an operation: its kind, its categories, its comment,
/// the counterparty it went through, hiding it, or the chain it is in. The
/// event itself is left as it came, so a refetch keeps the correction
/// (`trade_core::model::cryptotrack::CryptoOverride`).
#[tauri::command]
pub async fn crypto_set_override(
    input: CryptoSetOverrideInput,
    state: State<'_, AppState>,
) -> Result<CryptoSetOverrideResult, IpcError> {
    with_vault_mut(&state, |vault| {
        operations::crypto_set_override(vault, &input)
    })
}

ipc_commands!(cryptotrack_operations_commands, CRYPTOTRACK_OPERATIONS_NAMES, commands::cryptotrack::operations: [crypto_list_operations, crypto_operation_detail, crypto_set_override]);
