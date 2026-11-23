use crate::ipc::events::{AppEvent, VaultChanged, VaultSaveFailed};
use tauri::{AppHandle, Manager, State};
use trade_core::ipc_contract::events::VaultSaveFailedPayload;

use trade_core::ipc_contract::security::SecurityLevel;
use trade_core::ipc_contract::{
    ChangePasswordInput, CreateVaultInput, RemoveVaultPasswordInput, SetVaultPasswordInput,
    SetVaultPasswordResult, UnlockInput, VaultInfo, VaultStatus,
};
use trade_core::kernel::events::{LockReason, VaultLocked};
use trade_core::service::security_level::{check_new_secret, level_of_vault_file};
use trade_core::service::{CopyPolicy, UnlockedVault};

use crate::commands::{AppState, app_settings, lock_vault, require_vault};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use crate::state::LockBus;

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
pub async fn vault_status(state: State<'_, AppState>) -> Result<VaultStatus, IpcError> {
    status_of(&state)
}

pub(crate) fn status_of(state: &AppState) -> Result<VaultStatus, IpcError> {
    let mut path = state
        .vault_path
        .lock()
        .map_err(|_| "lock".to_string())?
        .clone();
    let guard = lock_vault(state)?;
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
    // Its callers are the background locks (idle, sleep, screen lock).
    if close_session_for(app, LockReason::Autolock).is_err() {
        return;
    }
    let state = app.state::<AppState>();
    if let Ok(status) = status_of(&state) {
        let _ = VaultChanged::emit(app, &status);
    }
}

#[tauri::command]
pub async fn create_vault(
    input: CreateVaultInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<VaultInfo, IpcError> {
    leave_sandbox(&app, &state);
    // A new vault with a password is level 2: a PIN of six digits or more.
    if let Some(pin) = input.password.as_deref() {
        check_new_secret(SecurityLevel::Pin, pin)?;
    }
    let path = crate::platform_fs::resolve_new_vault(&app, &input.path)?;
    close_open_vault(&app, &state, &path)?;
    // `None` = a vault without a password; an empty one is refused by the core.
    let mut vault = UnlockedVault::create_with(path.clone(), input.password.as_deref())?;
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
        trade_core::usecases::money::stores::update_store(
            &mut vault,
            &cash,
            name,
            "#16a34a",
            "",
            Default::default(),
        )?;
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
            .map_err(|_| IpcError::from(trade_core::Error::Crypto("lock poisoned".into())))?
            .as_ref()
            .expect("just stored"),
    );
    // Update vault_path and record in recents
    *state.vault_path.lock().map_err(|err| {
        IpcError::from(trade_core::Error::Crypto(format!("lock poisoned: {}", err)))
    })? = path.clone();
    crate::commands::vault_recents::add_recent(&app, &path.to_string_lossy());
    Ok(info)
}

#[tauri::command]
pub async fn unlock_vault(
    input: UnlockInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<VaultInfo, IpcError> {
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
) -> Result<VaultInfo, IpcError> {
    leave_sandbox(app, state);
    let path = crate::platform_fs::resolve_vault_to_open(app, &input.path)?;
    let pin_key = pin_guarded(&path);
    if let Some(key) = &pin_key {
        let wait = app_settings::current_settings(app).pin_retry_after(key, now_secs());
        if wait > 0 {
            let secs = u32::try_from(wait).unwrap_or(u32::MAX);
            return Err(IpcError::from(trade_core::Error::PinRetryLater { secs }));
        }
    }
    close_open_vault(app, state, &path)?;
    // A linked vault first takes in what changed in its external copy.
    let open_sync = crate::platform_fs::links::sync_before_open(app, &path)?;
    let device = crate::commands::app_settings::device_id(app);
    let opened = UnlockedVault::open_as(path.clone(), &input.password, Some(&device));
    if let Some(key) = &pin_key {
        note_pin_attempt(app, key, &opened);
    }
    let mut vault = opened?;
    // Edits made elsewhere reach the opened vault; the password is only
    // needed for this call and is not kept.
    if crate::platform_fs::links::take_in(app, &mut vault, open_sync, &input.password)?
        == crate::platform_fs::links::TakeIn::Reopen
    {
        drop(vault);
        vault = UnlockedVault::open_as(path.clone(), &input.password, Some(&device))?;
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
                IpcError::from(trade_core::Error::Crypto(format!("lock poisoned: {}", err)))
            })?
            .as_ref()
            .expect("just stored"),
    );
    // Update vault_path and record in recents
    *state.vault_path.lock().map_err(|err| {
        IpcError::from(trade_core::Error::Crypto(format!("lock poisoned: {}", err)))
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
pub async fn lock_vault_cmd(app: AppHandle) -> Result<(), IpcError> {
    close_session(&app).map_err(IpcError::from)
}

/// The single way a vault is closed: flush, snapshot, stop sync, drop, then
/// publish `VaultLocked`, which the wipe of decrypted attachments and the reset
/// of the live positions follow. Locking the sandbox is leaving it: the demo
/// data goes, nothing is flushed.
pub fn close_session(app: &AppHandle) -> trade_core::Result<()> {
    close_session_for(app, LockReason::Manual)
}

fn close_session_for(app: &AppHandle, reason: LockReason) -> trade_core::Result<()> {
    if leave_sandbox(app, &app.state::<AppState>()) {
        return Ok(());
    }
    close_in_order(&mut AppLockSteps { app }, reason)
}

/// Leave the sandbox if it is open, announcing it. Whether it was open.
pub(crate) fn leave_sandbox(app: &AppHandle, state: &AppState) -> bool {
    let left = crate::commands::sandbox::leave(app, state);
    announce_sandbox_leave(left, &mut AppLockSteps { app })
}

fn announce_sandbox_leave(left: bool, steps: &mut impl LockSteps) -> bool {
    if left {
        steps.publish(LockReason::SandboxLeave, true);
    }
    left
}

/// The steps of locking, so the order can be tested without Tauri.
trait LockSteps {
    /// Write the batched changes to disk.
    fn flush(&mut self) -> trade_core::Result<()>;
    /// The last word before the vault closes: what to remind of while locked.
    fn snapshot(&mut self);
    /// Stop sync.
    fn stop_sync(&mut self);
    /// Drop the unlocked vault.
    fn close(&mut self);
    /// Tell the app the lock happened (`VaultLocked`); `closed` is false when
    /// the vault stayed open.
    fn publish(&mut self, reason: LockReason, closed: bool);
}

/// Flush first: if the changes cannot be written, nothing is stopped and the
/// vault stays open with sync and the scheduler running. The lock is published
/// last either way: files decrypted for other programs must not linger.
fn close_in_order(steps: &mut impl LockSteps, reason: LockReason) -> trade_core::Result<()> {
    let flushed = steps.flush();
    let closed = flushed.is_ok();
    if closed {
        steps.snapshot();
        steps.stop_sync();
        steps.close();
    }
    steps.publish(reason, closed);
    flushed
}

/// Close the open vault, if any, before the one at `target` is stored.
fn close_open_vault(
    app: &AppHandle,
    state: &AppState,
    target: &std::path::Path,
) -> Result<(), IpcError> {
    let open = state
        .vault
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|vault| vault.path.clone()));
    close_if_open(open.as_deref(), target, &mut AppLockSteps { app })
        .map(drop)
        .map_err(IpcError::from)
}

/// Opening a vault while one is open here (the same file or another): close the
/// open one first (flush, stop sync, drop, publish). Otherwise its lock file (ours, so the new vault would not
/// own it) is deleted when it drops, and its deferred writer could write over
/// the new vault. Whether one was open.
fn close_if_open(
    open: Option<&std::path::Path>,
    target: &std::path::Path,
    steps: &mut impl LockSteps,
) -> trade_core::Result<bool> {
    let Some(open) = open else {
        return Ok(false);
    };
    let reason = if open == target {
        LockReason::Reopen
    } else {
        LockReason::Replace
    };
    close_in_order(steps, reason)?;
    Ok(true)
}

struct AppLockSteps<'a> {
    app: &'a AppHandle,
}

impl LockSteps for AppLockSteps<'_> {
    fn flush(&mut self) -> trade_core::Result<()> {
        // The guard is released inside `flush_now`, before the next steps.
        crate::vault_writer::flush_now(self.app).inspect_err(|err| {
            let _ =
                VaultSaveFailed::emit(self.app, &VaultSaveFailedPayload::Message(err.to_string()));
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

    fn publish(&mut self, reason: LockReason, closed: bool) {
        self.app
            .state::<LockBus>()
            .0
            .publish(&VaultLocked { reason, closed }.into());
    }
}

ipc_commands!(auth_commands, AUTH_NAMES, commands::auth: [vault_status, create_vault, unlock_vault, lock_vault_cmd, flush_vault, exit_without_saving, change_password, set_vault_password, remove_vault_password]);

#[cfg(test)]
mod lock_order_tests {
    use super::*;

    struct Recorder {
        calls: Vec<String>,
        flush_fails: bool,
    }

    impl Recorder {
        fn new(flush_fails: bool) -> Self {
            Self {
                calls: vec![],
                flush_fails,
            }
        }
    }

    impl LockSteps for Recorder {
        fn flush(&mut self) -> trade_core::Result<()> {
            self.calls.push("flush".into());
            if self.flush_fails {
                Err(trade_core::Error::Crypto("disk full".into()))
            } else {
                Ok(())
            }
        }
        fn snapshot(&mut self) {
            self.calls.push("snapshot".into());
        }
        fn stop_sync(&mut self) {
            self.calls.push("stop_sync".into());
        }
        fn close(&mut self) {
            self.calls.push("close".into());
        }
        fn publish(&mut self, reason: LockReason, closed: bool) {
            self.calls.push(format!("publish:{reason:?}:{closed}"));
        }
    }

    const CLOSED_IN_ORDER: [&str; 4] = ["flush", "snapshot", "stop_sync", "close"];

    fn order_with(publish: &str) -> Vec<String> {
        CLOSED_IN_ORDER
            .iter()
            .map(|step| (*step).to_string())
            .chain([publish.to_string()])
            .collect()
    }

    #[test]
    fn lock_flushes_before_snapshot_sync_stop_and_close_and_publishes_last() {
        let mut steps = Recorder::new(false);
        close_in_order(&mut steps, LockReason::Manual).unwrap();
        assert_eq!(steps.calls, order_with("publish:Manual:true"));
    }

    #[test]
    fn unlocking_while_a_vault_is_open_closes_it_first() {
        let mut steps = Recorder::new(false);
        let path = std::path::Path::new("/vaults/a.vault");
        assert!(close_if_open(Some(path), path, &mut steps).unwrap());
        assert_eq!(steps.calls, order_with("publish:Reopen:true"));
    }

    #[test]
    fn unlocking_with_nothing_open_closes_nothing() {
        let mut steps = Recorder::new(false);
        let path = std::path::Path::new("/vaults/a.vault");
        assert!(!close_if_open(None, path, &mut steps).unwrap());
        assert!(steps.calls.is_empty());
    }

    #[test]
    fn a_failed_flush_leaves_sync_and_the_vault_running() {
        let mut steps = Recorder::new(true);
        assert!(close_in_order(&mut steps, LockReason::Manual).is_err());
        assert_eq!(steps.calls, ["flush", "publish:Manual:false"]);
    }

    #[test]
    fn a_failed_flush_still_publishes_with_closed_false() {
        let mut steps = Recorder::new(true);
        assert!(close_in_order(&mut steps, LockReason::Autolock).is_err());
        assert_eq!(steps.calls.last().unwrap(), "publish:Autolock:false");
    }

    #[test]
    fn a_lock_publishes_vault_locked_on_every_close_path() {
        let published = |steps: Recorder| steps.calls.last().cloned().unwrap();

        let mut manual = Recorder::new(false);
        close_in_order(&mut manual, LockReason::Manual).unwrap();
        assert_eq!(published(manual), "publish:Manual:true");

        let mut autolock = Recorder::new(false);
        close_in_order(&mut autolock, LockReason::Autolock).unwrap();
        assert_eq!(published(autolock), "publish:Autolock:true");

        let mut sandbox = Recorder::new(false);
        assert!(announce_sandbox_leave(true, &mut sandbox));
        assert_eq!(sandbox.calls, ["publish:SandboxLeave:true"]);

        let mut not_sandbox = Recorder::new(false);
        assert!(!announce_sandbox_leave(false, &mut not_sandbox));
        assert!(not_sandbox.calls.is_empty());

        let open = std::path::Path::new("/vaults/a.vault");
        let mut reopen = Recorder::new(false);
        close_if_open(Some(open), open, &mut reopen).unwrap();
        assert_eq!(published(reopen), "publish:Reopen:true");

        let other = std::path::Path::new("/vaults/b.vault");
        let mut replace = Recorder::new(false);
        close_if_open(Some(open), other, &mut replace).unwrap();
        assert_eq!(published(replace), "publish:Replace:true");
    }
}

/// Put a freshly opened vault in place, writing the previous one's batched
/// changes first (switching vaults must not lose them).
fn store_vault(state: &AppState, vault: UnlockedVault) -> Result<(), IpcError> {
    let mut guard = state
        .vault
        .lock()
        .map_err(|_| IpcError::from(trade_core::Error::Crypto("lock poisoned".into())))?;
    if let Some(previous) = guard.as_mut() {
        previous.flush_pending()?;
    }
    *guard = Some(vault);
    // Another vault: the readings of the previous one are not its own.
    crate::cryptotrack::jobs::forget_live_positions();
    Ok(())
}

/// Write the batched changes now: the window went to the background (a phone
/// may kill the app there) or the user pressed «Retry» after a failed save.
#[tauri::command]
pub async fn flush_vault(app: AppHandle) -> Result<(), IpcError> {
    crate::vault_writer::flush_now(&app).map_err(IpcError::from)
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
) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().ok_or("Vault is locked")?;
    // A level-2 PIN changes to a PIN of six digits or more; the level-3 rule
    // is in the core, which keeps the cost of the key.
    if vault.security_level() == SecurityLevel::Pin {
        check_new_secret(SecurityLevel::Pin, &input.new_password)?;
    }
    let salt_before = vault.key_salt();
    let outcome = vault
        .change_password_with(
            &input.current_password,
            &input.new_password,
            CopyPolicy::default(),
        )
        .map(|report| report.warn_failed("change_password"));
    let key_changed = vault.key_salt() != salt_before;
    reset_binding_after(&outcome, key_changed, || {
        crate::biometric::system_gate(&app_settings::config_dir(&app))
            .disable(&vault.path.to_string_lossy());
    });
    outcome.map_err(IpcError::from)
}

/// The bound password is stale as soon as the key changed, whatever the
/// outcome of the steps after it: quick unlock starts over. A call that
/// failed before the key changed leaves the binding as it is.
fn reset_binding_after<T>(
    outcome: &trade_core::Result<T>,
    key_changed: bool,
    reset: impl FnOnce(),
) {
    if outcome.is_ok() || key_changed {
        reset();
    }
}

#[cfg(test)]
mod key_change_tests {
    use super::*;

    fn reset_called(outcome: trade_core::Result<()>, key_changed: bool) -> bool {
        let mut called = false;
        reset_binding_after(&outcome, key_changed, || called = true);
        called
    }

    #[test]
    fn the_biometric_binding_is_reset_after_a_key_change_even_when_the_tail_fails() {
        let tail_failure = Err(trade_core::Error::Io("copy cleanup".into()));
        assert!(reset_called(tail_failure, true));
        assert!(reset_called(Ok(()), true));
    }

    #[test]
    fn the_biometric_binding_stays_when_the_key_did_not_change() {
        let early_failure = Err(trade_core::Error::Secret);
        assert!(!reset_called(early_failure, false));
    }
}

/// Give the open vault that has no password one. The file is rewritten
/// encrypted, and a linked external copy gets the same bytes in that write; with
/// `delete_plain_copies` the vault's unencrypted `.bak` copies are deleted.
#[tauri::command]
pub async fn set_vault_password(
    input: SetVaultPasswordInput,
    state: State<'_, AppState>,
) -> Result<SetVaultPasswordResult, IpcError> {
    crate::commands::sandbox::refuse(&state, "a vault password")?;
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().ok_or("Vault is locked")?;
    check_new_secret(SecurityLevel::Pin, &input.new_password)?;
    let removed_copies = vault.set_password(&input.new_password, input.delete_plain_copies)?;
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
) -> Result<(), IpcError> {
    crate::commands::sandbox::refuse(&state, "a vault password")?;
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().ok_or("Vault is locked")?;
    if !input.external_copy_too
        && crate::platform_fs::links::is_linked(&app, &vault.path.to_string_lossy())
    {
        return Err(IpcError::from(trade_core::Error::InvalidInput(
            "the vault has an external copy that would lose its password too: confirm it".into(),
        )));
    }
    vault.remove_password(&input.current_password)?;
    crate::biometric::system_gate(&app_settings::config_dir(&app))
        .disable(&vault.path.to_string_lossy());
    Ok(())
}
