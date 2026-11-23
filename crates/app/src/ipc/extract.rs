//! Trial (core-ipc OD4): a `CommandArg` that holds the vault guard.
//!
//! Outcome, measured on tauri 2.12.5: it works in a sync command and in a
//! sync command marked `#[tauri::command(async)]` (runs on the async runtime,
//! off the main thread, but blocks a worker); it does not compile in an
//! `async fn` command: the guard is not `Send`.

use std::ops::Deref;
use std::sync::MutexGuard;

use tauri::Manager;
use tauri::Runtime;
use tauri::ipc::{CommandArg, CommandItem, InvokeError};
use trade_core::service::Session;

use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;

/// The unlocked vault, locked for the length of the command.
pub struct Unlocked<'guard>(MutexGuard<'guard, Option<Session>>);

impl<'guard> Unlocked<'guard> {
    fn open(state: &'guard AppState) -> Result<Self, IpcError> {
        Ok(Self(require_vault(state)?))
    }
}

impl Deref for Unlocked<'_> {
    type Target = Session;

    fn deref(&self) -> &Session {
        self.0.as_ref().expect("checked in open")
    }
}

impl<'guard, R: Runtime> CommandArg<'guard, R> for Unlocked<'guard> {
    fn from_command(command: CommandItem<'guard, R>) -> Result<Self, InvokeError> {
        let state = command
            .message
            .webview_ref()
            .try_state::<AppState>()
            .ok_or_else(|| InvokeError::from("AppState is not managed"))?;
        Unlocked::open(state.inner()).map_err(InvokeError::from)
    }
}

#[cfg(test)]
mod tests {
    use trade_core::error::Error;

    use super::*;
    use crate::commands::AppState;

    #[test]
    fn the_extractor_refuses_a_locked_vault() {
        let state = AppState::default();
        let outcome = Unlocked::open(&state);
        assert!(matches!(
            outcome,
            Err(IpcError::Core(Error::RequiresUnlock))
        ));
    }
}

#[cfg(test)]
mod shapes {
    use super::*;

    /// The real command shape that compiles: a sync body, off the main thread.
    #[tauri::command(async)]
    fn read_shape(vault: Unlocked<'_>) -> Result<usize, IpcError> {
        let _ = &*vault;
        Ok(1)
    }

    #[test]
    fn a_sync_command_can_take_the_extractor() {
        let _: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool> =
            Box::new(tauri::generate_handler![read_shape]);
    }
}
