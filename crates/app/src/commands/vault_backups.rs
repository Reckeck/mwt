//! Backup copies of the vault file: list, take one now, restore one, show
//! the folder. Logic lives in `trade_core::vault_io::backups` and
//! `UnlockedVault`; these are the IPC wrappers.

use tauri::State;
use trade_core::ipc_contract::{RestoreVaultBackupInput, VaultBackup};

use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;

/// Backups of the open vault, newest first.
#[tauri::command]
pub async fn list_vault_backups(state: State<'_, AppState>) -> Result<Vec<VaultBackup>, IpcError> {
    let guard = require_vault(&state)?;
    let files = guard.as_ref().unwrap().list_backups()?;
    Ok(files.into_iter().map(VaultBackup::from).collect())
}

/// Copy the vault file to a new backup now.
#[tauri::command]
pub async fn create_vault_backup(state: State<'_, AppState>) -> Result<VaultBackup, IpcError> {
    crate::commands::sandbox::refuse(&state, "a backup copy")?;
    let mut guard = require_vault(&state)?;
    let file = guard.as_mut().unwrap().create_backup()?;
    Ok(file.into())
}

/// Replace the vault's content with a backup. The state before is kept as a
/// backup of its own, returned here. The UI asks the user first.
#[tauri::command]
pub async fn restore_vault_backup(
    input: RestoreVaultBackupInput,
    state: State<'_, AppState>,
) -> Result<VaultBackup, IpcError> {
    crate::commands::sandbox::refuse(&state, "restoring a backup")?;
    let mut guard = require_vault(&state)?;
    let safety = guard.as_mut().unwrap().restore_backup(&input.name)?;
    Ok(safety.into())
}

/// Show the folder with the vault file in the system file manager (desktop).
#[tauri::command]
pub async fn reveal_vault_folder(state: State<'_, AppState>) -> Result<(), IpcError> {
    let path = {
        let guard = require_vault(&state)?;
        guard.as_ref().unwrap().path.clone()
    };
    state.opener.reveal_path(&path)
}
