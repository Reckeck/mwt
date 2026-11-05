//! Recent vaults persistence — plain JSON in app config dir.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, State};

use crate::commands::{AppState, err_to_string, lock_vault};
use trade_core::ipc_contract::{
    DeleteVaultFileInput, DeleteVaultFileOutput, ExportVaultCopyInput, PickedVault, RecentVault,
    RelocateRecentVaultInput, RestoreRecentVaultInput, SelectVaultInput, VaultStorage,
};
use trade_core::vault_io::delete::{
    Protected, delete_in_app_folder, ensure_vault_file, inside_folder, read_vault_bytes,
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
            encrypted: true,
            in_app_folder: false,
        })
        .collect()
}

/// Add a vault to recents (move to top, dedupe, cap). The sandbox is never added.
pub fn add_recent(app: &AppHandle, path: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sandbox = crate::commands::sandbox::dir(app).ok();
    let recents = opened(load_recents(app), path, now, sandbox.as_ref());
    save_recents(app, &recents);
}

/// `recents` after `path` was opened at `now` (seconds): it moves to the top,
/// the list is capped. The sandbox's file stays out of it.
fn opened(
    mut recents: Vec<RecentVault>,
    path: &str,
    now: u64,
    sandbox: Option<&trade_core::sandbox::SandboxDir>,
) -> Vec<RecentVault> {
    if sandbox.is_some_and(|dir| Path::new(path).starts_with(dir.root())) {
        return recents;
    }
    recents.retain(|recent| recent.path != path);
    recents.insert(
        0,
        RecentVault {
            path: path.to_string(),
            last_opened: now.to_string(),
            exists: Path::new(path).exists(),
            display_name: display_name_of(path),
            linked: false,
            encrypted: true,
            in_app_folder: false,
        },
    );
    recents.truncate(MAX_RECENTS);
    recents
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

/// `recents` with the row of `old` pointing at `new_path`: the row keeps its place
/// and `last_opened`; another row already at `new_path` is dropped, so no
/// duplicate appears. An unknown `old` leaves the list as it was.
fn relocated(mut recents: Vec<RecentVault>, old: &str, new_path: &str) -> Vec<RecentVault> {
    if old == new_path || !recents.iter().any(|recent| recent.path == old) {
        return recents;
    }
    recents.retain(|recent| recent.path != new_path);
    if let Some(row) = recents.iter_mut().find(|recent| recent.path == old) {
        row.path = new_path.to_string();
        row.display_name = display_name_of(new_path);
        row.exists = Path::new(new_path).exists();
    }
    recents
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
    let app_dir = crate::platform_fs::vaults_dir(&app).ok();
    for recent in &mut recents {
        recent.in_app_folder = app_dir
            .as_deref()
            .is_some_and(|dir| inside_folder(dir, Path::new(&recent.path)).is_some());
        recent.display_name = display_name_of(&recent.path);
        recent.linked = crate::platform_fs::links::is_linked(&app, &recent.path);
        recent.encrypted = trade_core::crypto::file_is_encrypted(Path::new(&recent.path));
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
        encrypted: true,
        in_app_folder: false,
    };
    insert_at(&mut recents, entry, input.index as usize, MAX_RECENTS);
    save_recents(&app, &recents);
    Ok(())
}

/// "Find the file…": the history row of a vanished file points at the file the
/// user found (the system dialog is opened by `pick_vault_file`).
#[tauri::command]
pub async fn relocate_recent_vault(
    app: AppHandle,
    input: RelocateRecentVaultInput,
) -> Result<(), String> {
    // The row may only point at a file that exists and is a vault: anything else is refused.
    ensure_vault_file(Path::new(&input.new_path)).map_err(err_to_string)?;
    let recents = relocated(load_recents(&app), &input.path, &input.new_path);
    save_recents(&app, &recents);
    Ok(())
}

/// Delete a vault from the app's own folder with its backup copies, and drop
/// it from recents. The path is checked in core (`vault_io::delete`); the open
/// vault and the sandbox are refused.
#[tauri::command]
pub async fn delete_vault_file(
    app: AppHandle,
    state: State<'_, AppState>,
    input: DeleteVaultFileInput,
) -> Result<DeleteVaultFileOutput, String> {
    let dir = crate::platform_fs::vaults_dir(&app).map_err(err_to_string)?;
    let sandbox = crate::commands::sandbox::dir(&app).map_err(err_to_string)?;
    let open_path = {
        let unlocked = lock_vault(&state).map_err(err_to_string)?.is_some();
        let path = state
            .vault_path
            .lock()
            .map_err(|_| "lock poisoned".to_string())?;
        unlocked.then(|| path.clone())
    };
    let protected = Protected {
        open_vault: open_path.as_deref(),
        sandbox: Some(sandbox.root()),
    };
    let done =
        delete_in_app_folder(&dir, Path::new(&input.path), protected).map_err(err_to_string)?;
    remove_recent(&app, &input.path);
    Ok(DeleteVaultFileOutput {
        file_name: done.file_name,
        backups: done.backups,
    })
}

/// Write the open vault's batched changes when `source` is that vault.
fn flush_if_open(
    open: Option<&mut trade_core::service::UnlockedVault>,
    source: &Path,
) -> trade_core::error::Result<()> {
    match open {
        Some(vault) if vault.path == source => vault.flush_pending(),
        _ => Ok(()),
    }
}

/// "Save a copy…": the vault file, byte for byte, to the place the system save
/// dialog picks (the frontend never names the path). Returns what to show as
/// "saved to"; `None` when the dialog was closed. The source itself and the
/// open vault are never overwritten.
#[tauri::command]
pub async fn export_vault_copy(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ExportVaultCopyInput,
) -> Result<Option<String>, String> {
    let source = Path::new(&input.path);
    // The copy is what is on disk: batched edits of the open vault go first.
    // The guard is dropped before the file is read and before the dialog.
    {
        let mut guard = state
            .vault
            .lock()
            .map_err(|_| "lock poisoned".to_string())?;
        flush_if_open(guard.as_mut(), source).map_err(err_to_string)?;
    }
    let bytes = read_vault_bytes(source).map_err(err_to_string)?;
    let open_path = state
        .vault_path
        .lock()
        .map_err(|_| "lock poisoned".to_string())?
        .clone();
    let name = display_name_of(&input.path);
    crate::platform_fs::save_vault_copy_dialog(&app, &name, &bytes, &[source, &open_path])
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn pick_vault_file(app: AppHandle, save: bool) -> Result<Option<PickedVault>, String> {
    crate::platform_fs::pick_vault(&app, save).map_err(crate::commands::err_to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use trade_core::service::{SaveHook, UnlockedVault};

    struct NoWake;

    impl SaveHook for NoWake {
        fn dirty(&self) {}
    }

    fn decimals_on_disk(path: &Path) -> u32 {
        trade_core::storage::VaultFile::open(path, "pw")
            .unwrap()
            .0
            .settings
            .display_decimals
    }

    #[test]
    fn a_copy_of_the_open_vault_holds_its_batched_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("home.wallet");
        let mut vault = UnlockedVault::create(path.clone(), "pw").unwrap();
        vault.set_write_behind(Some(std::sync::Arc::new(NoWake)));
        let before = decimals_on_disk(&path);
        let changed = before + 1;
        vault.doc.settings.display_decimals = changed;
        vault.persist().unwrap();
        assert_eq!(decimals_on_disk(&path), before, "batched, not written yet");

        let other = dir.path().join("other.wallet");
        flush_if_open(Some(&mut vault), &other).unwrap();
        assert_eq!(decimals_on_disk(&path), before, "another file: untouched");

        flush_if_open(Some(&mut vault), &path).unwrap();
        assert_eq!(decimals_on_disk(&path), changed);
        flush_if_open(None, &path).unwrap();
    }

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
    fn the_sandbox_is_not_a_recent_vault() {
        let cache = std::path::Path::new("/cache/app");
        let sandbox = trade_core::sandbox::SandboxDir::in_cache(cache);
        let file = sandbox.file().to_string_lossy().into_owned();
        let before = vec![rv("/home/me/main.wallet")];
        let after = opened(before.clone(), &file, 1, Some(&sandbox));
        assert_eq!(paths(&after), ["/home/me/main.wallet"]);
        // A real vault is added, on top; the list is capped.
        let after = opened(before, "/home/me/other.wallet", 2, Some(&sandbox));
        assert_eq!(
            paths(&after),
            ["/home/me/other.wallet", "/home/me/main.wallet"]
        );
        let many: Vec<_> = (0..MAX_RECENTS)
            .map(|number| rv(&format!("/v/{number}.wallet")))
            .collect();
        assert_eq!(opened(many, "/v/new.wallet", 3, None).len(), MAX_RECENTS);
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

    #[test]
    fn relocated_keeps_place_and_drops_duplicate() {
        let list = vec![rv("a"), rv("gone"), rv("c"), rv("found")];
        let after = relocated(list, "gone", "found");
        assert_eq!(paths(&after), ["a", "found", "c"]);
        let same = relocated(vec![rv("a")], "x", "y");
        assert_eq!(paths(&same), ["a"]);
    }

    #[test]
    fn relocation_target_must_be_a_vault_file() {
        let dir = std::env::temp_dir().join(format!("relocate-check-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let vault = dir.join("found.wallet");
        let mut bytes = trade_core::VAULT_MAGIC.to_vec();
        bytes.extend_from_slice(b"rest");
        std::fs::write(&vault, bytes).unwrap();
        let other = dir.join("notes.txt");
        std::fs::write(&other, b"not a vault at all").unwrap();

        assert!(ensure_vault_file(&vault).is_ok());
        for bad in [
            other.as_path(),
            dir.join("missing.wallet").as_path(),
            dir.as_path(),
        ] {
            let err = ensure_vault_file(bad).unwrap_err();
            assert!(err_to_string(err).starts_with("INVALID_INPUT"), "{bad:?}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The frontend sends `{ input: { path } }`: the command argument is named `input`.
    #[test]
    fn recents_commands_take_the_input_wrapper() {
        let args = serde_json::json!({ "input": { "path": "/v/a.wallet" } });
        let input: SelectVaultInput = serde_json::from_value(args["input"].clone()).unwrap();
        assert_eq!(input.path, "/v/a.wallet");
        let args = serde_json::json!({ "input": { "path": "/v/a", "new_path": "/w/a" } });
        let input: RelocateRecentVaultInput =
            serde_json::from_value(args["input"].clone()).unwrap();
        assert_eq!(input.new_path, "/w/a");
        // The bare `{ path }` form is what the old frontend sent: no `input` key to read.
        assert!(serde_json::json!({ "path": "/v/a" }).get("input").is_none());
    }
}
