//! Recent vaults persistence — plain JSON in app config dir.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

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
    read_recents(&recents_path(app))
}

/// Save recents to disk
fn save_recents(app: &AppHandle, recents: &[RecentVault]) {
    write_recents(&recents_path(app), recents);
}

fn read_recents(path: &Path) -> Vec<RecentVault> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn write_recents(path: &Path, recents: &[RecentVault]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        path,
        serde_json::to_string_pretty(recents).unwrap_or_default(),
    );
}

/// Point recents that lay in a moved app directory (the default vault) at
/// its new place. `config_dir` is the app config dir, before Tauri starts.
#[cfg(desktop)]
pub fn rebase_paths(config_dir: &Path, moves: &[(PathBuf, PathBuf)]) {
    let path = config_dir.join(RECENTS_FILE);
    let mut recents = read_recents(&path);
    let mut changed = false;
    for recent in &mut recents {
        if let Some(rebased) = trade_core::app_dirs::rebase_path(&recent.path, moves) {
            recent.exists = Path::new(&rebased).exists();
            recent.path = rebased;
            changed = true;
        }
    }
    if changed {
        write_recents(&path, &recents);
    }
}

/// Add a vault to recents (move to top, dedupe, cap)
pub fn add_recent(app: &AppHandle, path: &str) {
    let mut recents = load_recents(app);
    let path_str = path.to_string();
    recents.retain(|recent| recent.path != path_str);
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
    recents.retain(|recent| recent.path != path);
    save_recents(app, &recents);
}

// --- Tauri commands ---

#[tauri::command]
pub async fn recent_vaults(app: AppHandle) -> Result<Vec<RecentVault>, String> {
    let mut recents = load_recents(&app);
    // Refresh exists flag
    for recent in &mut recents {
        recent.exists = Path::new(&recent.path).exists();
    }
    save_recents(&app, &recents);
    Ok(recents)
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
            .add_filter("Multi Wallet Tracker vault", &["wallet"])
            .set_file_name("vault.wallet")
            .set_title("Сохранить vault как…")
    } else {
        dialog
            .add_filter("Multi Wallet Tracker vault", &["wallet"])
            .set_title("Открыть vault")
    };
    if !save {
        return Ok(dialog.blocking_pick_file().map(|path| path.to_string()));
    }
    // The save dialog returns exactly what was typed; a vault always gets its extension.
    let picked = dialog
        .blocking_save_file()
        .map(|path| path.into_path().map_err(|err| err.to_string()))
        .transpose()?;
    Ok(picked.map(|path| {
        trade_core::storage::ensure_wallet_ext(&path)
            .to_string_lossy()
            .into_owned()
    }))
}
