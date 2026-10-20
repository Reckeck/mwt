use tauri::{AppHandle, Emitter, Manager, State};

use trade_core::ipc_contract::{
    ChangePasswordInput, CreateVaultInput, UnlockInput, VaultInfo, VaultStatus,
};
use trade_core::service::UnlockedVault;

use crate::commands::{AppState, app_settings, err_to_string, lock_vault, require_vault};

/// Vault identity for the unlock screen. The base currency is *not* part of the
/// document any more — it is an app-level preference — so it is left out here.
fn info_of(vault: &UnlockedVault) -> VaultInfo {
    VaultInfo {
        path: vault.path.display().to_string(),
        doc_version: vault.doc.version,
        id: vault.doc.id.clone(),
        created_at: vault.doc.created_at.clone(),
        format: vault.fmt.label().to_string(),
        base_currency: vault.base_currency().to_string(),
    }
}

#[tauri::command]
pub async fn vault_status(state: State<'_, AppState>) -> Result<VaultStatus, String> {
    status_of(&state)
}

fn status_of(state: &AppState) -> Result<VaultStatus, String> {
    let path = state
        .vault_path
        .lock()
        .map_err(|_| "lock".to_string())?
        .clone();
    let guard = lock_vault(state).map_err(err_to_string)?;
    let unlocked = guard.is_some();
    let info = guard.as_ref().map(info_of);
    Ok(VaultStatus {
        path: path.display().to_string(),
        exists: path.exists(),
        unlocked,
        info,
        storage: crate::platform_fs::storage(),
    })
}

/// Lock without the window asking (the background lock, the button on the
/// sync notification): the window learns it from `vault-changed`.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn lock_now(app: &AppHandle) {
    // Unsaved changes first: if they cannot be written, the vault stays open.
    if let Err(err) = crate::vault_writer::flush_now(app) {
        let _ = app.emit(crate::vault_writer::SAVE_FAILED_EVENT, err.to_string());
        return;
    }
    crate::scheduler::snapshot_tick(app, true);
    crate::sync::on_lock(app);
    let state = app.state::<AppState>();
    if let Ok(mut guard) = lock_vault(&state) {
        *guard = None;
    }
    if let Ok(status) = status_of(&state) {
        let _ = app.emit("vault-changed", status);
    }
}

#[tauri::command]
pub async fn create_vault(
    input: CreateVaultInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let path = crate::platform_fs::resolve_new_vault(&app, &input.path).map_err(err_to_string)?;
    if input.password.is_empty() {
        return Err("INVALID_INPUT: password must not be empty".into());
    }
    let mut vault = UnlockedVault::create(path.clone(), &input.password)
        .map_err(crate::commands::err_to_string)?;
    crate::vault_writer::install(&mut vault);
    if let Some(name) = input
        .cash_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        && let Some(cash) = vault
            .doc
            .stores
            .iter()
            .find(|store| store.kind == trade_core::model::store::StoreKind::Cash)
            .map(|store| store.id.clone())
    {
        vault
            .update_store(&cash, name, "#16a34a", "", Default::default())
            .map_err(crate::commands::err_to_string)?;
    }
    store_vault(&state, vault)?;
    // The base currency lives in the app config, so push it into the new vault
    // before reading the info back out.
    app_settings::sync_base_currency(&state, &app);
    crate::sync::on_unlock(&app);
    let info = info_of(
        state
            .vault
            .lock()
            .map_err(|_| err_to_string(trade_core::Error::Crypto("lock poisoned".into())))?
            .as_ref()
            .expect("just stored"),
    );
    // Update vault_path and record in recents
    *state.vault_path.lock().map_err(|err| {
        err_to_string(trade_core::Error::Crypto(format!("lock poisoned: {}", err)))
    })? = path.clone();
    crate::commands::vault_recents::add_recent(&app, &path.to_string_lossy());
    Ok(info)
}

#[tauri::command]
pub async fn unlock_vault(
    input: UnlockInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let path =
        crate::platform_fs::resolve_vault_to_open(&app, &input.path).map_err(err_to_string)?;
    // A linked vault first takes in what changed in its external copy.
    crate::platform_fs::links::sync_before_open(&app, &path).map_err(err_to_string)?;
    let mut vault = UnlockedVault::open(path.clone(), &input.password)
        .map_err(crate::commands::err_to_string)?;
    crate::platform_fs::attach_mirror(&app, &mut vault);
    crate::vault_writer::install(&mut vault);
    store_vault(&state, vault)?;
    // The base currency lives in the app config, so push it into the opened
    // vault before reading the info back out.
    app_settings::sync_base_currency(&state, &app);
    crate::sync::on_unlock(&app);
    let info = info_of(
        state
            .vault
            .lock()
            .map_err(|err| {
                err_to_string(trade_core::Error::Crypto(format!("lock poisoned: {}", err)))
            })?
            .as_ref()
            .expect("just stored"),
    );
    // Update vault_path and record in recents
    *state.vault_path.lock().map_err(|err| {
        err_to_string(trade_core::Error::Crypto(format!("lock poisoned: {}", err)))
    })? = path.clone();
    crate::commands::vault_recents::add_recent(&app, &path.to_string_lossy());
    Ok(info)
}

#[tauri::command]
pub async fn lock_vault_cmd(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // The last word before the vault closes: what to remind of while locked.
    crate::scheduler::snapshot_tick(&app, true);
    crate::sync::on_lock(&app);
    let mut guard = lock_vault(&state).map_err(err_to_string)?;
    // Batched changes go to disk before the vault closes; a failed write keeps it open.
    if let Some(vault) = guard.as_mut() {
        vault.flush_pending().map_err(err_to_string)?;
    }
    *guard = None;
    Ok(())
}

/// Put a freshly opened vault in place, writing the previous one's batched
/// changes first (switching vaults must not lose them).
fn store_vault(state: &AppState, vault: UnlockedVault) -> Result<(), String> {
    let mut guard = state
        .vault
        .lock()
        .map_err(|_| err_to_string(trade_core::Error::Crypto("lock poisoned".into())))?;
    if let Some(previous) = guard.as_mut() {
        previous.flush_pending().map_err(err_to_string)?;
    }
    *guard = Some(vault);
    Ok(())
}

/// Write the batched changes now: the window went to the background (a phone
/// may kill the app there) or the user pressed «Retry» after a failed save.
#[tauri::command]
pub async fn flush_vault(app: AppHandle) -> Result<(), String> {
    crate::vault_writer::flush_now(&app).map_err(err_to_string)
}

/// Quit although the last changes could not be written (the user chose so).
#[tauri::command]
pub fn exit_without_saving(app: AppHandle) {
    crate::vault_writer::allow_exit_unsaved();
    app.exit(0);
}

#[tauri::command]
pub async fn change_password(
    input: ChangePasswordInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().ok_or("Vault is locked")?;
    vault
        .change_password(&input.current_password, &input.new_password)
        .map_err(crate::commands::err_to_string)
}
