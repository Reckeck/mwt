//! Recent vaults persistence — plain JSON in app config dir.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};
use trade_core::ipc_contract::{
    PickedVault, RecentVault, RestoreRecentVaultInput, SelectVaultInput, VaultStorage,
};
use trade_core::vault_io::display_name_of;

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

/// `.wallet` files in `dir` that are not in `known` yet, newest first.
fn stored_vaults(dir: &Path, known: &[RecentVault]) -> Vec<RecentVault> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(u64, String)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case(trade_core::storage::VAULT_EXT))
        })
        .map(|path| {
            let modified = std::fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |since| since.as_secs());
            (modified, path.to_string_lossy().into_owned())
        })
        .filter(|(_, path)| !known.iter().any(|recent| &recent.path == path))
        .collect();
    found.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    found
        .into_iter()
        .map(|(modified, path)| RecentVault {
            display_name: display_name_of(&path),
            path,
            last_opened: modified.to_string(),
            exists: true,
            linked: false,
        })
        .collect()
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
            display_name: display_name_of(path),
            linked: false,
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

/// Put `entry` back at `index`; past the end it goes last, a duplicate path is
/// never created (the row stays where it is), the list stays within `max`.
fn insert_at(recents: &mut Vec<RecentVault>, entry: RecentVault, index: usize, max: usize) {
    if recents.iter().any(|recent| recent.path == entry.path) {
        return;
    }
    recents.insert(index.min(recents.len()), entry);
    recents.truncate(max);
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
    // In app storage the user cannot browse for a vault, so every one there is listed.
    if crate::platform_fs::storage() == VaultStorage::App
        && let Ok(dir) = crate::platform_fs::vaults_dir(&app)
    {
        recents.extend(stored_vaults(&dir, &recents));
    }
    for recent in &mut recents {
        recent.display_name = display_name_of(&recent.path);
        recent.linked = crate::platform_fs::links::is_linked(&app, &recent.path);
    }
    Ok(recents)
}

#[tauri::command]
pub async fn forget_recent_vault(app: AppHandle, input: SelectVaultInput) -> Result<(), String> {
    remove_recent(&app, &input.path);
    Ok(())
}

/// Undo of `forget_recent_vault`: the row returns to its former place.
#[tauri::command]
pub async fn restore_recent_vault(
    app: AppHandle,
    input: RestoreRecentVaultInput,
) -> Result<(), String> {
    let mut recents = load_recents(&app);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let entry = RecentVault {
        path: input.path.clone(),
        last_opened: now.to_string(),
        exists: Path::new(&input.path).exists(),
        display_name: display_name_of(&input.path),
        linked: false,
    };
    insert_at(&mut recents, entry, input.index as usize, MAX_RECENTS);
    save_recents(&app, &recents);
    Ok(())
}

#[tauri::command]
pub async fn pick_vault_file(app: AppHandle, save: bool) -> Result<Option<PickedVault>, String> {
    crate::platform_fs::pick_vault(&app, save).map_err(crate::commands::err_to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rv(path: &str) -> RecentVault {
        RecentVault {
            path: path.into(),
            ..Default::default()
        }
    }

    fn paths(list: &[RecentVault]) -> Vec<&str> {
        list.iter().map(|recent| recent.path.as_str()).collect()
    }

    #[test]
    fn insert_at_restores_position() {
        let mut list = vec![rv("a"), rv("c")];
        insert_at(&mut list, rv("b"), 1, 10);
        assert_eq!(paths(&list), ["a", "b", "c"]);
        insert_at(&mut list, rv("z"), 0, 10);
        assert_eq!(paths(&list), ["z", "a", "b", "c"]);
    }

    #[test]
    fn insert_at_past_end_goes_last() {
        let mut list = vec![rv("a")];
        insert_at(&mut list, rv("b"), 99, 10);
        assert_eq!(paths(&list), ["a", "b"]);
    }

    #[test]
    fn insert_at_skips_duplicate() {
        let mut list = vec![rv("a"), rv("b")];
        insert_at(&mut list, rv("b"), 0, 10);
        assert_eq!(paths(&list), ["a", "b"]);
    }

    #[test]
    fn insert_at_respects_cap() {
        let mut list = vec![rv("a"), rv("b")];
        insert_at(&mut list, rv("c"), 2, 2);
        assert_eq!(paths(&list), ["a", "b"]);
    }
}
