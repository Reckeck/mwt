//! Backup copies of the vault file: list, take one now, restore one, show
//! the folder. Logic lives in `trade_core::vault_io::backups` and
//! `UnlockedVault`; these are the IPC wrappers.

use tauri::State;
use trade_core::error::Error;
use trade_core::ipc_contract::{RestoreVaultBackupInput, VaultBackup};

use crate::commands::{AppState, err_to_string, require_vault};

/// Backups of the open vault, newest first.
#[tauri::command]
pub async fn list_vault_backups(state: State<'_, AppState>) -> Result<Vec<VaultBackup>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let files = guard
        .as_ref()
        .unwrap()
        .list_backups()
        .map_err(err_to_string)?;
    Ok(files.into_iter().map(VaultBackup::from).collect())
}

/// Copy the vault file to a new backup now.
#[tauri::command]
pub async fn create_vault_backup(state: State<'_, AppState>) -> Result<VaultBackup, String> {
    crate::commands::sandbox::refuse(&state, "a backup copy")?;
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let file = guard
        .as_mut()
        .unwrap()
        .create_backup()
        .map_err(err_to_string)?;
    Ok(file.into())
}

/// Replace the vault's content with a backup. The state before is kept as a
/// backup of its own, returned here. The UI asks the user first.
#[tauri::command]
pub async fn restore_vault_backup(
    input: RestoreVaultBackupInput,
    state: State<'_, AppState>,
) -> Result<VaultBackup, String> {
    crate::commands::sandbox::refuse(&state, "restoring a backup")?;
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let safety = guard
        .as_mut()
        .unwrap()
        .restore_backup(&input.name)
        .map_err(err_to_string)?;
    Ok(safety.into())
}

/// Show the folder with the vault file in the system file manager (desktop).
#[tauri::command]
pub async fn reveal_vault_folder(state: State<'_, AppState>) -> Result<(), String> {
    let path = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        guard.as_ref().unwrap().path.clone()
    };
    #[cfg(desktop)]
    {
        tauri_plugin_opener::reveal_item_in_dir(&path)
            .map_err(|err| err_to_string(Error::Io(err.to_string())))
    }
    #[cfg(mobile)]
    {
        let _ = path;
        Err(err_to_string(Error::Unsupported))
    }
}
