use tauri::{AppHandle, Emitter, Manager, State};

use trade_core::ipc_contract::security::SecurityLevel;
use trade_core::ipc_contract::{
    ChangePasswordInput, CreateVaultInput, RemoveVaultPasswordInput, SetVaultPasswordInput,
    SetVaultPasswordResult, UnlockInput, VaultInfo, VaultStatus,
};
use trade_core::service::UnlockedVault;
use trade_core::service::security_level::{check_new_secret, level_of_vault_file};

use crate::commands::{AppState, app_settings, err_to_string, lock_vault, require_vault};

/// Vault identity for the unlock screen. The base currency is *not* part of the
/// document any more — it is an app-level preference — so it is left out here.
pub fn info_of(vault: &UnlockedVault) -> VaultInfo {
    VaultInfo {
        path: vault.path.display().to_string(),
        doc_version: vault.doc.version,
        id: vault.doc.id.clone(),
        created_at: vault.doc.created_at.clone(),
        format: vault.fmt.label().to_string(),
        base_currency: vault.base_currency().to_string(),
        encrypted: vault.is_encrypted(),
    }
}

#[tauri::command]
pub async fn vault_status(state: State<'_, AppState>) -> Result<VaultStatus, String> {
    status_of(&state)
}

pub(crate) fn status_of(state: &AppState) -> Result<VaultStatus, String> {
    let mut path = state
        .vault_path
        .lock()
        .map_err(|_| "lock".to_string())?
        .clone();
    let guard = lock_vault(state).map_err(err_to_string)?;
    // The sandbox keeps the real path as the "last vault"; what is open is its own file.
    if crate::commands::sandbox::is_active(state)
        && let Some(vault) = guard.as_ref()
    {
        path.clone_from(&vault.path);
    }
    let unlocked = guard.is_some();
    let info = guard.as_ref().map(info_of);
    let encrypted = info.as_ref().map_or_else(
        || trade_core::crypto::file_is_encrypted(&path),
        |info| info.encrypted,
    );
    Ok(VaultStatus {
        path: path.display().to_string(),
        exists: path.exists(),
        unlocked,
        info,
        storage: crate::platform_fs::storage(),
        encrypted,
        sandbox: crate::commands::sandbox::is_active(state),
    })
}

/// Lock without the window asking (the background lock, the button on the
/// sync notification): the window learns it from `vault-changed`.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn lock_now(app: &AppHandle) {
    // Locking the sandbox is leaving it: the demo data goes.
    if crate::commands::sandbox::leave(app, &app.state::<AppState>()) {
        if let Ok(status) = status_of(&app.state::<AppState>()) {
            let _ = app.emit("vault-changed", status);
        }
        return;
    }
    if close_in_order(&mut AppLockSteps { app }).is_err() {
        return;
    }
    let state = app.state::<AppState>();
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
    crate::commands::sandbox::leave(&app, &state);
    // A new vault with a password is level 2: a PIN of six digits or more.
    if let Some(pin) = input.password.as_deref() {
        check_new_secret(SecurityLevel::Pin, pin).map_err(err_to_string)?;
    }
    let path = crate::platform_fs::resolve_new_vault(&app, &input.path).map_err(err_to_string)?;
    // `None` = a vault without a password; an empty one is refused by the core.
    let mut vault = UnlockedVault::create_with(path.clone(), input.password.as_deref())
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
    if let Some(crypto) = input.crypto {
        vault
            .update_settings(trade_core::ipc_contract::UpdateSettingsInput {
                crypto: Some(crypto),
                ..Default::default()
            })
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
    unlock_with(input, &app, &state, true).await
}

/// The unlock path shared by the password and the biometric entry. `typed` is
/// whether a person typed the password: only then the 7-day biometric window
/// starts over (a password released by the biometrics is not a new entry).
pub(crate) async fn unlock_with(
    input: UnlockInput,
    app: &AppHandle,
    state: &AppState,
    typed: bool,
) -> Result<VaultInfo, String> {
    crate::commands::sandbox::leave(app, state);
    let path =
        crate::platform_fs::resolve_vault_to_open(app, &input.path).map_err(err_to_string)?;
    let pin_key = pin_guarded(&path);
    if let Some(key) = &pin_key {
        let wait = app_settings::current_settings(app).pin_retry_after(key, now_secs());
        if wait > 0 {
            let secs = u32::try_from(wait).unwrap_or(u32::MAX);
            return Err(err_to_string(trade_core::Error::PinRetryLater { secs }));
        }
    }
    let open_path = state
        .vault
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|vault| vault.path.clone()));
    close_if_open_at(open_path.as_deref(), &path, &mut AppLockSteps { app })
        .map_err(err_to_string)?;
    // A linked vault first takes in what changed in its external copy.
    let open_sync =
        crate::platform_fs::links::sync_before_open(app, &path).map_err(err_to_string)?;
    let device = crate::commands::app_settings::device_id(app);
    let opened = UnlockedVault::open_as(path.clone(), &input.password, Some(&device));
    if let Some(key) = &pin_key {
        note_pin_attempt(app, key, &opened);
    }
    let mut vault = opened.map_err(crate::commands::err_to_string)?;
    // Edits made elsewhere reach the opened vault; the password is only
    // needed for this call and is not kept.
    if crate::platform_fs::links::take_in(app, &mut vault, open_sync, &input.password)
        .map_err(err_to_string)?
        == crate::platform_fs::links::TakeIn::Reopen
    {
        drop(vault);
        vault = UnlockedVault::open_as(path.clone(), &input.password, Some(&device))
            .map_err(crate::commands::err_to_string)?;
    }
    crate::platform_fs::attach_mirror(app, &mut vault);
    crate::vault_writer::install(&mut vault);
    store_vault(state, vault)?;
    // The base currency lives in the app config, so push it into the opened
    // vault before reading the info back out.
    app_settings::sync_base_currency(state, app);
    crate::sync::on_unlock(app);
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
    crate::commands::vault_recents::add_recent(app, &path.to_string_lossy());
    if typed {
        crate::biometric::system_gate(&app_settings::config_dir(app))
            .password_entered(&path.to_string_lossy());
    }
    Ok(info)
}

/// The key under which wrong PINs for the vault at `path` are counted, when
/// it is a level-2 vault (the delay is for a short PIN, not a long password).
fn pin_guarded(path: &std::path::Path) -> Option<String> {
    level_of_vault_file(path)
        .is_ok_and(|level| level == SecurityLevel::Pin)
        .then(|| path.to_string_lossy().into_owned())
}

/// Count a wrong PIN, or forget the count once the vault opened. Any other
/// failure (a busy file, too little memory) is not a guess and counts nothing.
fn note_pin_attempt<T>(app: &AppHandle, key: &str, opened: &trade_core::Result<T>) {
    let mut settings = app_settings::current_settings(app);
    let changed = match opened {
        Ok(_) => settings.reset_pin_failures(key),
        Err(trade_core::Error::Secret) => {
            settings.record_pin_failure(key, now_secs());
            true
        }
        Err(_) => false,
    };
    if changed {
        let _ = app_settings::save(&app_settings::config_dir(app), &settings);
    }
}

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[tauri::command]
pub async fn lock_vault_cmd(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // Locking the sandbox is leaving it: the demo data goes, nothing is kept.
    if crate::commands::sandbox::leave(&app, &state) {
        return Ok(());
    }
    close_in_order(&mut AppLockSteps { app: &app }).map_err(err_to_string)
}

/// The four steps of locking, so the order can be tested without Tauri.
trait LockSteps {
    /// Write the batched changes to disk.
    fn flush(&mut self) -> trade_core::Result<()>;
    /// The last word before the vault closes: what to remind of while locked.
    fn snapshot(&mut self);
    /// Stop sync.
    fn stop_sync(&mut self);
    /// Drop the unlocked vault.
    fn close(&mut self);
}

/// Flush first: if the changes cannot be written, nothing is stopped and the
/// vault stays open with sync and the scheduler running.
fn close_in_order(steps: &mut impl LockSteps) -> trade_core::Result<()> {
    steps.flush()?;
    steps.snapshot();
    steps.stop_sync();
    steps.close();
    Ok(())
}

/// Opening a vault that is already open here: close the open one first (flush,
/// stop sync, drop). Otherwise its lock file (ours, so the new vault would not
/// own it) is deleted when it drops, and its deferred writer could write over
/// the new vault. Whether it was open at `path`.
fn close_if_open_at(
    open: Option<&std::path::Path>,
    path: &std::path::Path,
    steps: &mut impl LockSteps,
) -> trade_core::Result<bool> {
    if open != Some(path) {
        return Ok(false);
    }
    close_in_order(steps)?;
    Ok(true)
}

struct AppLockSteps<'a> {
    app: &'a AppHandle,
}

impl LockSteps for AppLockSteps<'_> {
    fn flush(&mut self) -> trade_core::Result<()> {
        // The guard is released inside `flush_now`, before the next steps.
        crate::vault_writer::flush_now(self.app).inspect_err(|err| {
            let _ = self
                .app
                .emit(crate::vault_writer::SAVE_FAILED_EVENT, err.to_string());
        })
    }

    fn snapshot(&mut self) {
        crate::scheduler::snapshot_tick(self.app, true);
    }

    fn stop_sync(&mut self) {
        crate::sync::on_lock(self.app);
    }

    fn close(&mut self) {
        let state = self.app.state::<AppState>();
        if let Ok(mut guard) = lock_vault(&state) {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod lock_order_tests {
    use super::*;

    struct Recorder {
        calls: Vec<&'static str>,
        flush_fails: bool,
    }

    impl LockSteps for Recorder {
        fn flush(&mut self) -> trade_core::Result<()> {
            self.calls.push("flush");
            if self.flush_fails {
                Err(trade_core::Error::Crypto("disk full".into()))
            } else {
                Ok(())
            }
        }
        fn snapshot(&mut self) {
            self.calls.push("snapshot");
        }
        fn stop_sync(&mut self) {
            self.calls.push("stop_sync");
        }
        fn close(&mut self) {
            self.calls.push("close");
        }
    }

    #[test]
    fn lock_flushes_before_snapshot_sync_stop_and_close() {
        let mut steps = Recorder {
            calls: vec![],
            flush_fails: false,
        };
        close_in_order(&mut steps).unwrap();
        assert_eq!(steps.calls, ["flush", "snapshot", "stop_sync", "close"]);
    }

    #[test]
    fn unlocking_the_vault_that_is_open_closes_it_first() {
        let path = std::path::Path::new("/tmp/a.wallet");
        let mut steps = Recorder {
            calls: vec![],
            flush_fails: false,
        };
        assert!(close_if_open_at(Some(path), path, &mut steps).unwrap());
        assert_eq!(steps.calls, ["flush", "snapshot", "stop_sync", "close"]);
    }

    #[test]
    fn unlocking_another_vault_or_none_closes_nothing() {
        let mut steps = Recorder {
            calls: vec![],
            flush_fails: false,
        };
        let other = std::path::Path::new("/tmp/b.wallet");
        assert!(!close_if_open_at(None, other, &mut steps).unwrap());
        assert!(
            !close_if_open_at(
                Some(std::path::Path::new("/tmp/a.wallet")),
                other,
                &mut steps
            )
            .unwrap()
        );
        assert!(steps.calls.is_empty());
    }

    #[test]
    fn a_failed_flush_leaves_sync_and_the_vault_running() {
        let mut steps = Recorder {
            calls: vec![],
            flush_fails: true,
        };
        assert!(close_in_order(&mut steps).is_err());
        assert_eq!(steps.calls, ["flush"]);
    }
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
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().ok_or("Vault is locked")?;
    // A level-2 PIN changes to a PIN of six digits or more; the level-3 rule
    // is in the core, which keeps the cost of the key.
    if vault.security_level() == SecurityLevel::Pin {
        check_new_secret(SecurityLevel::Pin, &input.new_password).map_err(err_to_string)?;
    }
    vault
        .change_password(&input.current_password, &input.new_password)
        .map_err(crate::commands::err_to_string)?;
    // The bound password is stale now: quick unlock starts over.
    crate::biometric::system_gate(&app_settings::config_dir(&app))
        .disable(&vault.path.to_string_lossy());
    Ok(())
}

/// Give the open vault that has no password one. The file is rewritten
/// encrypted, and a linked external copy gets the same bytes in that write; with
/// `delete_plain_copies` the vault's unencrypted `.bak` copies are deleted.
#[tauri::command]
pub async fn set_vault_password(
    input: SetVaultPasswordInput,
    state: State<'_, AppState>,
) -> Result<SetVaultPasswordResult, String> {
    crate::commands::sandbox::refuse(&state, "a vault password")?;
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().ok_or("Vault is locked")?;
    check_new_secret(SecurityLevel::Pin, &input.new_password).map_err(err_to_string)?;
    let removed_copies = vault
        .set_password(&input.new_password, input.delete_plain_copies)
        .map_err(err_to_string)?;
    Ok(SetVaultPasswordResult { removed_copies })
}

/// Take the password off the open vault. The current password is checked in
/// Rust first. A vault with an external copy keeps its password unless the
/// caller confirms the copy loses it too (`external_copy_too`).
#[tauri::command]
pub async fn remove_vault_password(
    input: RemoveVaultPasswordInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    crate::commands::sandbox::refuse(&state, "a vault password")?;
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().ok_or("Vault is locked")?;
    if !input.external_copy_too
        && crate::platform_fs::links::is_linked(&app, &vault.path.to_string_lossy())
    {
        return Err(err_to_string(trade_core::Error::InvalidInput(
            "the vault has an external copy that would lose its password too: confirm it".into(),
        )));
    }
    vault
        .remove_password(&input.current_password)
        .map_err(err_to_string)?;
    crate::biometric::system_gate(&app_settings::config_dir(&app))
        .disable(&vault.path.to_string_lossy());
    Ok(())
}
