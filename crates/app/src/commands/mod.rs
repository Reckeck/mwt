//! IPC command layer — thin translation between `trade-core` services and
//! the frontend. See `docs/ipc.md` for the full contract.

pub mod analytics;
pub mod app_settings;
pub mod auth;
pub mod calc;
pub mod categories;
pub mod contacts;
pub mod enrichment;
pub mod exchange;
pub mod losses;
pub mod notifications;
pub mod planning;
pub mod portfolio;
pub mod pricing;
pub mod settings;
pub mod stores;
pub mod transactions;
pub mod vault_recents;

use std::path::PathBuf;
use std::sync::Mutex;

use trade_core::error::Error;
use trade_core::pricing::PricingService;
use trade_core::service::UnlockedVault;

/// Global application state managed by Tauri.
pub struct AppState {
    pub vault: Mutex<Option<UnlockedVault>>,
    pub pricing: PricingService,
    pub vault_path: Mutex<PathBuf>,
    /// Notices told while the vault is locked (see `notify_store`).
    pub notify: Mutex<crate::notify_store::NotifyMemory>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            vault: Mutex::new(None),
            pricing: PricingService::default(),
            vault_path: Mutex::new(PathBuf::new()),
            notify: Mutex::new(crate::notify_store::NotifyMemory::default()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a core error into a Tauri-compatible error string (code-prefixed).
pub fn err_to_string(err: Error) -> String {
    format!("{}: {}", err.code(), err)
}

/// Grab a locked vault; returns an error if the app is locked.
pub fn lock_vault<'a>(
    state: &'a AppState,
) -> Result<std::sync::MutexGuard<'a, Option<UnlockedVault>>, Error> {
    state
        .vault
        .lock()
        .map_err(|_| Error::Crypto("lock poisoned".into()))
}

pub fn require_vault(
    state: &AppState,
) -> Result<std::sync::MutexGuard<'_, Option<UnlockedVault>>, Error> {
    let guard = lock_vault(state)?;
    if guard.is_none() {
        return Err(Error::RequiresUnlock);
    }
    Ok(guard)
}
