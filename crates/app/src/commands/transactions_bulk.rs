//! Several operations at once (Ops-1 · 4). Each command returns what its undo
//! needs: deleted operations go back through `restore_transactions`, old
//! categories and wallets through the same command again.

use tauri::State;

use trade_core::service::{DeletedTransaction, TxCategories, TxStore};
use trade_core::usecases::money::bulk;

use crate::commands::AppState;
use crate::commands::with_vault_mut;
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

#[tauri::command]
pub async fn delete_transactions(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<DeletedTransaction>, IpcError> {
    with_vault_mut(&state, |vault| bulk::delete_transactions(vault, &ids))
}

#[tauri::command]
pub async fn restore_transactions(
    items: Vec<DeletedTransaction>,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| bulk::restore_transactions(vault, items))
}

#[tauri::command]
pub async fn set_transaction_categories(
    changes: Vec<TxCategories>,
    state: State<'_, AppState>,
) -> Result<Vec<TxCategories>, IpcError> {
    with_vault_mut(&state, |vault| {
        bulk::set_transaction_categories(vault, changes)
    })
}

#[tauri::command]
pub async fn move_transactions(
    changes: Vec<TxStore>,
    state: State<'_, AppState>,
) -> Result<Vec<TxStore>, IpcError> {
    with_vault_mut(&state, |vault| bulk::move_transactions(vault, changes))
}

ipc_commands!(transactions_bulk_commands, TRANSACTIONS_BULK_NAMES, commands::transactions_bulk: [delete_transactions, restore_transactions, set_transaction_categories, move_transactions]);
