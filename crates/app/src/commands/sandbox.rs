//! The sandbox (docs/ipc.md, «sandbox»): a temporary vault with demo data to try
//! the app without touching real data. The work is in `trade_core::sandbox`;
//! this file opens and closes it, erases it, and says what is switched off
//! while it is open.
//!
//! In the sandbox: nothing goes to `recent_vaults.json`, the path of the
//! last opened vault stays as it was, sync, handing the vault over, data
//! sources, exchange keys, external copies and system notifications are off.
//! Automations still run on the demo data; they just tell nobody outside.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager, State};
use trade_core::Error;
use trade_core::ipc_contract::VaultInfo;
use trade_core::sandbox::SandboxDir;

use crate::commands::auth::info_of;
use crate::commands::{AppState, app_settings, err_to_string, lock_vault};

/// The sandbox folder in this app's cache directory (one per channel: the
/// release and the dev build have their own).
pub fn dir(app: &AppHandle) -> Result<SandboxDir, Error> {
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|err| Error::Sandbox(format!("no cache directory: {err}")))?;
    Ok(SandboxDir::in_cache(&cache))
}

/// Today in UTC: every demo operation is dated before it, so none is in the
/// future in any time zone.
fn today() -> chrono::NaiveDate {
    chrono::Utc::now().date_naive()
}

/// Whether the open vault is the sandbox.
pub fn is_active(state: &AppState) -> bool {
    state.sandbox.load(Ordering::SeqCst)
}

/// For what is switched off in the sandbox: `Err(SANDBOX)` while it is open.
pub fn refuse(state: &AppState, what: &str) -> Result<(), String> {
    if is_active(state) {
        return Err(err_to_string(Error::Sandbox(format!(
            "{what} is off in the sandbox"
        ))));
    }
    Ok(())
}

/// [`refuse`] for a command that has the app handle, not the state.
pub fn refuse_app(app: &AppHandle, what: &str) -> Result<(), String> {
    refuse(&app.state::<AppState>(), what)
}

/// Whether the sandbox is open, asked from the app handle (jobs, sync).
pub fn is_active_app(app: &AppHandle) -> bool {
    is_active(&app.state::<AppState>())
}

/// At start, before any vault is opened: what a crash left is erased.
pub fn erase_on_start(app: &AppHandle) {
    match dir(app).and_then(|sandbox| sandbox.erase()) {
        Ok(()) => {}
        Err(err) => eprintln!("sandbox: could not erase the leftovers: {err}"),
    }
}

/// At exit: close the sandbox vault and erase the folder.
pub fn erase_on_exit(app: &AppHandle) {
    let state = app.state::<AppState>();
    close_vault(&state);
    erase_on_start(app);
}

/// Drop the sandbox vault, if that is what is open. A real vault is never touched.
fn close_vault(state: &AppState) {
    if !is_active(state) {
        return;
    }
    if let Ok(mut guard) = lock_vault(state) {
        *guard = None;
    }
    state.sandbox.store(false, Ordering::SeqCst);
}

/// Leave the sandbox: close its vault and erase its folder. `false` when it
/// was not open (nothing was done).
pub fn leave(app: &AppHandle, state: &AppState) -> bool {
    if !is_active(state) {
        return false;
    }
    close_vault(state);
    erase_on_start(app);
    true
}

/// A fresh sandbox vault in place of whatever sandbox was open.
fn open_fresh(app: &AppHandle, state: &AppState) -> Result<VaultInfo, String> {
    let sandbox = dir(app).map_err(err_to_string)?;
    let info = {
        let mut guard = lock_vault(state).map_err(err_to_string)?;
        // Opening replaces the sandbox, never a real vault: that one stays.
        if guard.is_some() && !is_active(state) {
            return Err(err_to_string(Error::Sandbox(
                "lock the vault before opening the sandbox".into(),
            )));
        }
        // The old sandbox vault closes first: its file is erased and written again.
        *guard = None;
        let mut vault = sandbox.create(today()).map_err(err_to_string)?;
        crate::vault_writer::install(&mut vault);
        let info = info_of(&vault);
        *guard = Some(vault);
        state.sandbox.store(true, Ordering::SeqCst);
        info
    };
    // The base currency is the app's, as for any opened vault.
    app_settings::sync_base_currency(state, app);
    Ok(info)
}

/// «Открыть песочницу»: a new vault with twelve months of demo data. The
/// real vault must be locked. Not in the recent vaults, not the last opened.
#[tauri::command]
pub async fn open_sandbox(app: AppHandle, state: State<'_, AppState>) -> Result<VaultInfo, String> {
    open_fresh(&app, &state)
}

/// «Сбросить»: the demo data as it was at the start. Only while the
/// sandbox is open, and only if the open vault is the file in its folder.
#[tauri::command]
pub async fn reset_sandbox(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let sandbox = dir(&app).map_err(err_to_string)?;
    {
        let guard = lock_vault(&state).map_err(err_to_string)?;
        let inside = is_active(&state)
            && guard
                .as_ref()
                .is_some_and(|vault| sandbox.contains(&vault.path).is_ok());
        if !inside {
            return Err(err_to_string(Error::Sandbox(
                "the sandbox is not open".into(),
            )));
        }
    }
    open_fresh(&app, &state)
}

/// «Выйти»: back to the unlock screen, the demo data erased.
#[tauri::command]
pub async fn close_sandbox(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if leave(&app, &state) {
        Ok(())
    } else {
        Err(err_to_string(Error::Sandbox(
            "the sandbox is not open".into(),
        )))
    }
}

/// Whether the open vault is the sandbox.
#[tauri::command]
pub async fn is_sandbox(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(is_active(&state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use trade_core::service::UnlockedVault;

    fn state_with_vault(dir: &std::path::Path, sandbox: bool) -> AppState {
        let state = AppState::new();
        let vault = UnlockedVault::create_with(dir.join("some.wallet"), None).unwrap();
        *state.vault.lock().unwrap() = Some(vault);
        state.sandbox.store(sandbox, Ordering::SeqCst);
        state
    }

    #[test]
    fn what_is_off_in_the_sandbox_says_so() {
        let state = AppState::new();
        assert!(refuse(&state, "sync").is_ok());
        state.sandbox.store(true, Ordering::SeqCst);
        let message = refuse(&state, "sync").unwrap_err();
        assert!(message.starts_with("SANDBOX: "), "{message}");
    }

    #[test]
    fn closing_never_drops_a_real_vault() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_vault(dir.path(), false);
        close_vault(&state);
        assert!(state.vault.lock().unwrap().is_some());
        state.sandbox.store(true, Ordering::SeqCst);
        close_vault(&state);
        assert!(state.vault.lock().unwrap().is_none());
        assert!(!is_active(&state));
    }
}
