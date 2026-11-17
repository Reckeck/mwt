//! Several operations at once (Ops-1 · 4). Each command returns what its undo
//! needs: deleted operations go back through `restore_transactions`, old
//! categories and wallets through the same command again.

use tauri::State;

use trade_core::service::{DeletedTransaction, TxCategories, TxStore};

use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;

#[tauri::command]
pub async fn delete_transactions(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<DeletedTransaction>, IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .delete_transactions(&ids)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn restore_transactions(
    items: Vec<DeletedTransaction>,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .restore_transactions(items)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn set_transaction_categories(
    changes: Vec<TxCategories>,
    state: State<'_, AppState>,
) -> Result<Vec<TxCategories>, IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .set_transaction_categories(changes)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn move_transactions(
    changes: Vec<TxStore>,
    state: State<'_, AppState>,
) -> Result<Vec<TxStore>, IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .move_transactions(changes)
        .map_err(IpcError::from)
}
