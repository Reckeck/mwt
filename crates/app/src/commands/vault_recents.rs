//! Recent vaults persistence — plain JSON in app config dir.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::commands::{AppState, err_to_string, require_vault};
use trade_core::ipc_contract::{RecentVault, SelectVaultInput};

const RECENTS_FILE: &str = "recent_vaults.json";
const MAX_RECENTS: usize = 10;

/// Get app config directory path for recents file
fn recents_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .expect("app config dir")
        .join(RECENTS_FILE)
}

/// Load recents from disk
fn load_recents(app: &AppHandle) -> Vec<RecentVault> {
    let path = recents_path(app);
    if !path.exists() {
        return Vec::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save recents to disk
fn save_recents(app: &AppHandle, recents: &[RecentVault]) {
    let path = recents_path(app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(recents).unwrap_or_default(),
    );
}

/// Add a vault to recents (move to top, dedupe, cap)
pub fn add_recent(app: &AppHandle, path: &str) {
    let mut recents = load_recents(app);
    let path_str = path.to_string();
    recents.retain(|r| r.path != path_str);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    recents.insert(
        0,
        RecentVault {
            path: path_str,
            last_opened: now.to_string(),
            exists: Path::new(path).exists(),
        },
    );
    if recents.len() > MAX_RECENTS {
        recents.truncate(MAX_RECENTS);
    }
    save_recents(app, &recents);
}

/// Remove a vault from recents
fn remove_recent(app: &AppHandle, path: &str) {
    let mut recents = load_recents(app);
    recents.retain(|r| r.path != path);
    save_recents(app, &recents);
}

// --- Tauri commands ---

#[tauri::command]
pub async fn recent_vaults(app: AppHandle) -> Result<Vec<RecentVault>, String> {
    let mut recents = load_recents(&app);
    // Refresh exists flag
    for r in &mut recents {
        r.exists = Path::new(&r.path).exists();
    }
    save_recents(&app, &recents);
    Ok(recents)
}

#[tauri::command]
pub async fn select_vault(
    input: SelectVaultInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<trade_core::ipc_contract::VaultStatus, String> {
    let path = PathBuf::from(&input.path);
    if !path.exists() {
        return Err("NOT_FOUND: vault file does not exist".into());
    }
    // If vault is unlocked, we need to lock it first before switching
    let mut g = require_vault(&state).map_err(err_to_string)?;
    if g.is_some() {
        *g = None; // Lock current vault
    }
    // Update vault_path in AppState
    *state
        .vault_path
        .lock()
        .map_err(|e| err_to_string(trade_core::Error::Crypto(format!("lock poisoned: {}", e))))? =
        path.clone();
    // Return status for the new path
    let g = require_vault(&state).map_err(err_to_string)?;
    let unlocked = g.is_some();
    let info = g.as_ref().map(|v| trade_core::ipc_contract::VaultInfo {
        path: v.path.display().to_string(),
        doc_version: v.doc.version,
        id: v.doc.id.clone(),
        created_at: v.doc.created_at.clone(),
        format: v.fmt.label().to_string(),
        base_currency: v.doc.settings.base_currency.clone(),
    });
    add_recent(&app, &input.path);
    Ok(trade_core::ipc_contract::VaultStatus {
        path: path.display().to_string(),
        exists: true,
        unlocked,
        info,
    })
}

#[tauri::command]
pub async fn forget_recent_vault(app: AppHandle, input: SelectVaultInput) -> Result<(), String> {
    remove_recent(&app, &input.path);
    Ok(())
}

#[tauri::command]
pub async fn pick_vault_file(app: AppHandle, save: bool) -> Result<Option<String>, String> {
    let dialog = app.dialog().file();
    let dialog = if save {
        dialog
            .add_filter("TradeApp Vault", &["wallet"])
            .set_title("Сохранить vault как…")
    } else {
        dialog
            .add_filter("TradeApp Vault", &["wallet"])
            .set_title("Открыть vault")
    };
    let path = if save {
        dialog.blocking_save_file()
    } else {
        dialog.blocking_pick_file()
    };
    Ok(path.map(|p| p.to_string()))
}
