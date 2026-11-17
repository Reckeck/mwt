//! The run-time helpers behind every `#[ipc]` command: sandbox refusal, a
//! blocking thread, the vault lock and error conversion, in one fixed order.
//! They are plain functions over [`VaultHost`], so tests need no Tauri runtime.

use tauri::{AppHandle, Manager};
use tracing::{Instrument, field, info_span};
use trade_core::Error;
use trade_core::service::UnlockedVault;

use crate::commands::sandbox::refuse;
use crate::commands::{AppState, lock_vault};
use crate::ipc::IpcError;

/// Wire text of the error a panicking body turns into.
const INTERNAL_ERROR: &str = "internal error";

/// Where a command finds the application state.
pub trait VaultHost: Clone + Send + 'static {
    fn app_state(&self) -> &AppState;
}

/// The Tauri handle reaches the state it manages (`AppState` is registered
/// with `Builder::manage`); `State::inner` keeps the borrow of the handle.
impl VaultHost for AppHandle {
    fn app_state(&self) -> &AppState {
        self.state::<AppState>().inner()
    }
}

/// What a command declares about how it runs.
#[derive(Debug, Clone, Copy)]
pub struct Gate {
    /// Refuse while the sandbox is open.
    pub sandbox_deny: bool,
}

/// One argument of a command: its name and how to print its type as TS.
#[derive(Debug, Clone, Copy)]
pub struct ArgMeta {
    pub name: &'static str,
    pub ts: fn() -> String,
}

/// What a command is: its name, ordered arguments and the TS result type.
#[derive(Debug, Clone, Copy)]
pub struct CommandMeta {
    pub name: &'static str,
    pub args: &'static [ArgMeta],
    pub ret: fn() -> String,
}

/// Run a body that only reads the open vault.
pub async fn read<H, R>(
    host: H,
    meta: &CommandMeta,
    gate: Gate,
    body: impl FnOnce(&UnlockedVault) -> Result<R, IpcError> + Send + 'static,
) -> Result<R, IpcError>
where
    H: VaultHost,
    R: Send + 'static,
{
    run(host, meta, gate, move |state| {
        let guard = lock_vault(state)?;
        body(guard.as_ref().ok_or(Error::RequiresUnlock)?)
    })
    .await
}

/// Run a body that may change the open vault.
pub async fn write<H, R>(
    host: H,
    meta: &CommandMeta,
    gate: Gate,
    body: impl FnOnce(&mut UnlockedVault) -> Result<R, IpcError> + Send + 'static,
) -> Result<R, IpcError>
where
    H: VaultHost,
    R: Send + 'static,
{
    run(host, meta, gate, move |state| {
        let mut guard = lock_vault(state)?;
        body(guard.as_mut().ok_or(Error::RequiresUnlock)?)
    })
    .await
}

/// Run a body that works with or without a vault: it gets the state itself.
pub async fn locked_ok<H, R>(
    host: H,
    meta: &CommandMeta,
    gate: Gate,
    body: impl FnOnce(&AppState) -> Result<R, IpcError> + Send + 'static,
) -> Result<R, IpcError>
where
    H: VaultHost,
    R: Send + 'static,
{
    run(host, meta, gate, body).await
}

/// The shared pipeline inside a span named after the command: sandbox
/// refusal, `spawn_blocking`, then the step (lock, body); a failure puts its
/// `code` on the span.
async fn run<H, R>(
    host: H,
    meta: &CommandMeta,
    gate: Gate,
    step: impl FnOnce(&AppState) -> Result<R, IpcError> + Send + 'static,
) -> Result<R, IpcError>
where
    H: VaultHost,
    R: Send + 'static,
{
    let span = info_span!("ipc", command = meta.name, code = field::Empty);
    let outcome = execute(host, meta, gate, step)
        .instrument(span.clone())
        .await;
    if let Err(error) = &outcome {
        span.record("code", error.code());
    }
    outcome
}

async fn execute<H, R>(
    host: H,
    meta: &CommandMeta,
    gate: Gate,
    step: impl FnOnce(&AppState) -> Result<R, IpcError> + Send + 'static,
) -> Result<R, IpcError>
where
    H: VaultHost,
    R: Send + 'static,
{
    if gate.sandbox_deny {
        refuse(host.app_state(), meta.name)?;
    }
    tokio::task::spawn_blocking(move || step(host.app_state()))
        .await
        .unwrap_or_else(|_| Err(IpcError::from(INTERNAL_ERROR)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::thread;

    use super::*;
    use crate::commands::err_to_string;

    #[derive(Clone)]
    struct TestHost(Arc<AppState>);

    impl VaultHost for TestHost {
        fn app_state(&self) -> &AppState {
            &self.0
        }
    }

    const META: CommandMeta = CommandMeta {
        name: "test_command",
        args: &[],
        ret: String::new,
    };
    const OPEN: Gate = Gate {
        sandbox_deny: false,
    };

    fn locked_host() -> TestHost {
        TestHost(Arc::new(AppState::new()))
    }

    fn unlocked_host() -> (TestHost, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = UnlockedVault::create(dir.path().join("a.wallet"), "pw").expect("vault");
        let host = locked_host();
        *host.0.vault.lock().expect("lock") = Some(vault);
        (host, dir)
    }

    #[tokio::test]
    async fn read_on_a_locked_vault_is_requires_unlock() {
        let error = read(locked_host(), &META, OPEN, |_| Ok(()))
            .await
            .unwrap_err();
        assert_eq!(error.code(), "REQUIRES_UNLOCK");
    }

    #[tokio::test]
    async fn read_hands_the_open_vault_to_the_body() {
        let (host, _dir) = unlocked_host();
        let dirty = read(host, &META, OPEN, |vault| Ok(vault.has_unsaved_changes())).await;
        assert!(!dirty.expect("runs"));
    }

    #[tokio::test]
    async fn write_marks_nothing_on_an_error() {
        let (host, _dir) = unlocked_host();
        let state = host.0.clone();
        let error = write(host, &META, OPEN, |_| -> Result<(), IpcError> {
            Err(IpcError::from("nope"))
        })
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "nope");
        let guard = state.vault.lock().expect("lock");
        assert!(!guard.as_ref().expect("still open").has_unsaved_changes());
    }

    #[tokio::test]
    async fn sandbox_deny_refuses_in_the_sandbox_with_the_sandbox_code() {
        let host = locked_host();
        host.0.sandbox.store(true, Ordering::SeqCst);
        let gate = Gate { sandbox_deny: true };
        let error = locked_ok(host, &META, gate, |_| Ok(())).await.unwrap_err();
        assert_eq!(error.code(), "SANDBOX");
        assert_eq!(
            error.to_string(),
            err_to_string(Error::Sandbox("test_command is off in the sandbox".into()))
        );
    }

    #[tokio::test]
    async fn locked_ok_runs_without_a_vault() {
        let value = locked_ok(locked_host(), &META, OPEN, |_| Ok(7)).await;
        assert_eq!(value.expect("runs"), 7);
    }

    #[tokio::test]
    async fn the_body_runs_off_the_runtime_thread() {
        let caller = thread::current().id();
        let body = locked_ok(locked_host(), &META, OPEN, |_| Ok(thread::current().id())).await;
        assert_ne!(body.expect("runs"), caller);
    }

    #[tokio::test]
    async fn a_panic_in_the_body_is_an_error_not_a_hang() {
        let outcome = locked_ok(locked_host(), &META, OPEN, |_| -> Result<(), IpcError> {
            panic!("boom")
        })
        .await;
        let error = outcome.unwrap_err();
        assert!(matches!(&error, IpcError::Message(text) if text == "internal error"));
    }

    #[tokio::test]
    async fn a_poisoned_lock_is_a_crypto_error() {
        let host = locked_host();
        let state = host.0.clone();
        let _ = thread::spawn(move || {
            let _guard = state.vault.lock().expect("lock");
            panic!("poison it");
        })
        .join();
        let error = read(host, &META, OPEN, |_| Ok(())).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            err_to_string(Error::Crypto("lock poisoned".into()))
        );
    }
}
