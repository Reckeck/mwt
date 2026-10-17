//! The external copy of a vault: link a document, unlink it, show how the
//! last copy went, settle a `SYNC_CONFLICT`. Logic lives in
//! `platform_fs::links`; these are the IPC wrappers.

use std::path::PathBuf;

use tauri::{AppHandle, State};
use trade_core::error::Error;
use trade_core::ipc_contract::{LinkVaultInput, ResolveVaultConflictInput, VaultLinkStatus};

use crate::commands::{AppState, err_to_string, require_vault};
use crate::platform_fs::{self, links};

/// Path of the unlocked vault; the guard is gone when this returns, so a
/// dialog can stay open without blocking other commands (AGENTS.md rule 3).
fn unlocked_path(state: &AppState) -> Result<PathBuf, String> {
    let guard = require_vault(state).map_err(err_to_string)?;
    Ok(guard
        .as_ref()
        .map(|vault| vault.path.clone())
        .unwrap_or_default())
}

/// Re-read the link for the unlocked vault and start or stop its copying.
fn reattach(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let mut guard = require_vault(state).map_err(err_to_string)?;
    if let Some(vault) = guard.as_mut() {
        platform_fs::attach_mirror(app, vault);
    }
    Ok(())
}

#[tauri::command]
pub async fn vault_link_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<VaultLinkStatus, String> {
    let path = unlocked_path(&state)?;
    Ok(links::get(&app, &path)
        .map(|link| link.status())
        .unwrap_or_default())
}

/// Pick or create the document and write the vault there right away.
/// Returns `None` when the dialog is cancelled.
#[tauri::command]
pub async fn link_vault_external(
    input: LinkVaultInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<VaultLinkStatus>, String> {
    let path = unlocked_path(&state)?;
    let name = trade_core::vault_io::display_name_of(&path.to_string_lossy());
    let Some(target) =
        platform_fs::pick_link_target(&app, input.existing, &name).map_err(err_to_string)?
    else {
        return Ok(None);
    };
    if target.as_path() == Some(path.as_path()) {
        return Err(err_to_string(Error::InvalidInput(
            "the copy must be a different file than the vault".into(),
        )));
    }
    // A previous link gives its grant back before the new one is kept.
    links::remove(&app, &path);
    let link = links::VaultLink {
        target: target.to_string(),
        ..Default::default()
    };
    links::set(&app, &path, link.clone());
    let bytes = std::fs::read(&path).map_err(|err| err_to_string(err.into()))?;
    links::push(&app, &path, &link, &bytes);
    reattach(&app, &state)?;
    let status = links::get(&app, &path).map(|link| link.status());
    if let Some(error) = status.as_ref().and_then(|status| status.last_error.clone()) {
        return Err(err_to_string(Error::Io(error)));
    }
    Ok(status)
}

#[tauri::command]
pub async fn unlink_vault_external(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let path = unlocked_path(&state)?;
    links::remove(&app, &path);
    reattach(&app, &state)
}

/// After `SYNC_CONFLICT` on unlock: keep one side, then unlock again.
#[tauri::command]
pub async fn resolve_vault_conflict(
    input: ResolveVaultConflictInput,
    app: AppHandle,
) -> Result<(), String> {
    let path = platform_fs::resolve_vault_to_open(&app, &input.path).map_err(err_to_string)?;
    links::resolve_conflict(&app, &path, input.keep).map_err(err_to_string)
}
