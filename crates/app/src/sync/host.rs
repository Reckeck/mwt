//! What a sync link needs from the app, as a trait: the app implements it
//! with its handle, the tests with two vaults in memory and a real socket.

use std::sync::MutexGuard;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use trade_core::appconfig::AppSettings;
use trade_core::error::{Error, Result};
use trade_core::model::notify::NoticeRecord;
use trade_core::service::UnlockedVault;
use trade_core::sync::session::DeviceKey;

use super::{Hub, SyncHub};
use crate::commands::{AppState, require_vault};

pub(crate) trait Host: Clone + Send + Sync + 'static {
    fn hub(&self) -> MutexGuard<'_, Hub>;
    fn with_vault<T>(&self, work: impl FnOnce(&mut UnlockedVault) -> Result<T>) -> Result<T>;
    fn settings(&self) -> AppSettings;
    fn device_key(&self) -> Result<DeviceKey>;
    fn key_storage(&self) -> &'static str;
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S);
    /// Show notices that arrived by sync (the system notice follows focus).
    fn deliver(&self, notices: Vec<NoticeRecord>);
    /// The previous main device handed its role over.
    fn become_main(&self);
    /// Hand the main role over (the preferred device is here).
    fn demote(&self);
    /// The link to the main device closed.
    fn main_lost(&self, generation: u64);
}

pub(crate) fn lock_hub(hub: &SyncHub) -> MutexGuard<'_, Hub> {
    // A poisoned hub is still usable: its data stays consistent between calls.
    match hub.inner.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

impl Host for AppHandle {
    fn hub(&self) -> MutexGuard<'_, Hub> {
        lock_hub(self.state::<SyncHub>().inner())
    }

    fn with_vault<T>(&self, work: impl FnOnce(&mut UnlockedVault) -> Result<T>) -> Result<T> {
        let state = self.state::<AppState>();
        let mut guard = require_vault(&state)?;
        let vault = guard.as_mut().ok_or(Error::RequiresUnlock)?;
        work(vault)
    }

    fn settings(&self) -> AppSettings {
        crate::commands::app_settings::current_settings(self)
    }

    fn device_key(&self) -> Result<DeviceKey> {
        super::keystore::device_key(self).map(|(key, _)| key)
    }

    fn key_storage(&self) -> &'static str {
        super::keystore::storage(self)
    }

    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        let _ = Emitter::emit(self, event, payload);
    }

    fn deliver(&self, notices: Vec<NoticeRecord>) {
        crate::scheduler::deliver(self, notices, false);
    }

    fn become_main(&self) {
        super::discovery::become_main(self);
    }

    fn demote(&self) {
        super::discovery::demote(self);
    }

    fn main_lost(&self, generation: u64) {
        super::discovery::main_lost(self, generation);
    }
}
