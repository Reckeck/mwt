use tauri::State;

use trade_core::ipc_contract::{
    ChangePasswordInput, CreateVaultInput, UnlockInput, VaultInfo, VaultStatus,
};
use trade_core::service::UnlockedVault;

use crate::commands::{AppState, err_to_string, lock_vault, require_vault};

fn info_of(v: &UnlockedVault) -> VaultInfo {
    VaultInfo {
        path: v.path.display().to_string(),
        doc_version: v.doc.version,
        id: v.doc.id.clone(),
        created_at: v.doc.created_at.clone(),
        format: v.fmt.label().to_string(),
        base_currency: v.doc.settings.base_currency.clone(),
    }
}

#[tauri::command]
pub async fn vault_status(state: State<'_, AppState>) -> Result<VaultStatus, String> {
    let path = state
        .vault_path
        .lock()
        .map_err(|_| "lock".to_string())?
        .clone();
    let g = lock_vault(&state).map_err(err_to_string)?;
    let unlocked = g.is_some();
    let info = g.as_ref().map(info_of);
    Ok(VaultStatus {
        path: path.display().to_string(),
        exists: path.exists(),
        unlocked,
        info,
    })
}

#[tauri::command]
pub async fn create_vault(
    input: CreateVaultInput,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let path = std::path::PathBuf::from(&input.path);
    if input.password.is_empty() {
        return Err("INVALID_INPUT: password must not be empty".into());
    }
    let vault =
        UnlockedVault::create(path, &input.password).map_err(crate::commands::err_to_string)?;
    let info = info_of(&vault);
    *state.vault.lock().map_err(|_| "lock".to_string())? = Some(vault);
    Ok(info)
}

#[tauri::command]
pub async fn unlock_vault(
    input: UnlockInput,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let path = std::path::PathBuf::from(&input.path);
    let vault =
        UnlockedVault::open(path, &input.password).map_err(crate::commands::err_to_string)?;
    let info = info_of(&vault);
    *state.vault.lock().map_err(|_| "lock".to_string())? = Some(vault);
    Ok(info)
}

#[tauri::command]
pub async fn lock_vault_cmd(state: State<'_, AppState>) -> Result<(), String> {
    let mut g = lock_vault(&state).map_err(err_to_string)?;
    *g = None;
    Ok(())
}

#[tauri::command]
pub async fn change_password(
    input: ChangePasswordInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if input.new_password.is_empty() {
        return Err("INVALID_INPUT: new password must not be empty".into());
    }
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().ok_or("Vault is locked")?;
    v.change_password(&input.new_password)
        .map_err(crate::commands::err_to_string)
}
