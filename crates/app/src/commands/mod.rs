//! IPC command layer — thin translation between `trade-core` services and
//! the frontend. See `docs/ipc.md` for the full contract.

pub mod analytics;
pub mod app_settings;
pub mod attachments;
pub mod auth;
pub mod calc;
pub mod categories;
pub mod contacts;
pub mod cryptotrack;
pub mod enrichment;
pub mod exchange;
pub mod losses;
pub mod mobile;
pub mod notifications;
pub mod planning;
pub mod portfolio;
pub mod pricing;
pub mod rates;
pub mod sandbox;
pub mod security;
pub mod settings;
pub mod shared_events;
pub mod sources;
pub mod stores;
pub mod sync;
pub mod transactions;
pub mod transactions_bulk;
pub mod transfer;
pub mod updates;
pub mod vault_backups;
pub mod vault_link;
pub mod vault_recents;
pub mod workflows;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use mwt_base::error::Error;
use mwt_cryptotrack::tokens::known;
use mwt_net::PricingService;
use mwt_usecases::service::Session;
use tauri::State;

use crate::ipc::IpcError;
use crate::ipc::opener::Opener;

/// Global application state managed by Tauri.
pub struct AppState {
    pub vault: Mutex<Option<Session>>,
    pub pricing: PricingService,
    pub vault_path: Mutex<PathBuf>,
    /// Notices told while the vault is locked (see `notify_store`).
    pub notify: Mutex<crate::notify_store::NotifyMemory>,
    /// The open vault is the sandbox (see `commands::sandbox`).
    pub sandbox: AtomicBool,
    /// The only way to open a link or a file in the system.
    pub opener: Opener,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            vault: Mutex::new(None),
            pricing: PricingService::default().with_known_tokens(known::is_known),
            vault_path: Mutex::new(PathBuf::new()),
            notify: Mutex::new(crate::notify_store::NotifyMemory::default()),
            sandbox: AtomicBool::new(false),
            opener: Opener::system(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Grab a locked vault; returns an error if the app is locked.
pub fn lock_vault<'a>(
    state: &'a AppState,
) -> Result<std::sync::MutexGuard<'a, Option<Session>>, Error> {
    state
        .vault
        .lock()
        .map_err(|_| Error::Crypto("lock poisoned".into()))
}

pub fn require_vault(
    state: &AppState,
) -> Result<std::sync::MutexGuard<'_, Option<Session>>, Error> {
    let guard = lock_vault(state)?;
    if guard.is_none() {
        return Err(Error::RequiresUnlock);
    }
    Ok(guard)
}

/// Run `read` against the open vault. `E` is `IpcError`, or `String` for a
/// command not yet converted (the shim goes with T24).
pub(crate) fn with_vault<T, E: From<IpcError>>(
    state: &State<'_, AppState>,
    read: impl FnOnce(&Session) -> mwt_base::Result<T>,
) -> Result<T, E> {
    let guard = require_vault(state).map_err(IpcError::from)?;
    Ok(read(guard.as_ref().unwrap()).map_err(IpcError::from)?)
}

/// Run `write` against the open vault; `E` as in `with_vault`.
pub(crate) fn with_vault_mut<T, E: From<IpcError>>(
    state: &State<'_, AppState>,
    write: impl FnOnce(&mut Session) -> mwt_base::Result<T>,
) -> Result<T, E> {
    let mut guard = require_vault(state).map_err(IpcError::from)?;
    Ok(write(guard.as_mut().unwrap()).map_err(IpcError::from)?)
}
