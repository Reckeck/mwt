//! The open vault's writer: changes are batched and written in one go.
//!
//! A mutation only marks the vault dirty (`UnlockedVault::persist` with a
//! hook installed, see `trade_core::service::saving`). This thread waits for
//! a quiet moment — [`IDLE`] after the last change, at most [`CAP`] after the
//! first — then snapshots the document under the vault lock and seals and
//! writes it outside, so a burst of edits costs one write and no command waits
//! behind the disk. Closing paths (exit, lock, switching vaults, the window
//! hidden) write at once through [`flush_now`].

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use trade_core::service::{SaveHook, UnlockedVault};
use trade_core::{Error, Result};

use crate::commands::AppState;

/// Quiet time after the last change before the batch is written.
const IDLE: Duration = Duration::from_millis(500);
/// Longest a change waits while edits keep coming.
const CAP: Duration = Duration::from_secs(3);
/// Pauses between attempts after a failed write.
const BACKOFF: [Duration; 4] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
];

/// Event: a batched write failed (payload: the error text); retried by itself.
pub const SAVE_FAILED_EVENT: &str = "vault-save-failed";
/// Event: writing works again after a failure.
pub const SAVED_EVENT: &str = "vault-saved";

struct Wake(Mutex<Sender<()>>);

impl SaveHook for Wake {
    fn dirty(&self) {
        if let Ok(sender) = self.0.lock() {
            let _ = sender.send(());
        }
    }
}

static HOOK: OnceLock<Arc<Wake>> = OnceLock::new();
/// The user chose to quit although the last changes could not be written.
static EXIT_ANYWAY: AtomicBool = AtomicBool::new(false);

/// Start the writer thread (once, at app start).
pub fn spawn(app: AppHandle) {
    let (sender, receiver) = mpsc::channel();
    if HOOK.set(Arc::new(Wake(Mutex::new(sender)))).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("vault-writer".into())
        .spawn(move || run(&app, &receiver))
        .expect("spawn the vault writer");
}

/// Batch the saves of a vault that is about to be stored in the app state.
pub fn install(vault: &mut UnlockedVault) {
    if let Some(hook) = HOOK.get() {
        vault.set_write_behind(Some(hook.clone() as Arc<dyn SaveHook>));
    }
}

/// Write the open vault's batched changes now (under the vault lock).
pub fn flush_now(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let mut guard = state
        .vault
        .lock()
        .map_err(|_| Error::Crypto("lock poisoned".into()))?;
    match guard.as_mut() {
        Some(vault) => vault.flush_pending(),
        None => Ok(()),
    }
}

/// Quit without the last changes (asked for after a failed write on exit).
pub fn allow_exit_unsaved() {
    EXIT_ANYWAY.store(true, Ordering::SeqCst);
}

pub fn exit_unsaved_allowed() -> bool {
    EXIT_ANYWAY.load(Ordering::SeqCst)
}

fn run(app: &AppHandle, receiver: &Receiver<()>) {
    // Sleep until the first change of a batch, then until the batch is quiet.
    while receiver.recv().is_ok() {
        if !wait_for_quiet(receiver, IDLE, CAP) {
            return;
        }
        write_with_retries(app, receiver);
    }
}

/// After the first change: wait until `idle` passes without another one, or
/// `cap` since the first. `false` when the app is going away.
fn wait_for_quiet(receiver: &Receiver<()>, idle: Duration, cap: Duration) -> bool {
    let first = Instant::now();
    loop {
        let left = cap.saturating_sub(first.elapsed());
        if left.is_zero() {
            return true;
        }
        match receiver.recv_timeout(idle.min(left)) {
            Ok(()) => continue,
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

fn write_with_retries(app: &AppHandle, receiver: &Receiver<()>) {
    let mut failed = false;
    for attempt in 0.. {
        match write_batch(app) {
            Ok(()) => {
                if failed {
                    let _ = app.emit(SAVED_EVENT, ());
                }
                return;
            }
            Err(err) => {
                if !failed {
                    eprintln!("vault: save failed, will retry: {err}");
                    let _ = app.emit(SAVE_FAILED_EVENT, err.to_string());
                    failed = true;
                }
                std::thread::sleep(BACKOFF[attempt.min(BACKOFF.len() - 1)]);
                // Changes made meanwhile ride along with the retry.
                while receiver.try_recv().is_ok() {}
            }
        }
    }
}

/// Snapshot under the lock, seal and write outside it, settle under it again.
fn write_batch(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let job = {
        let mut guard = state
            .vault
            .lock()
            .map_err(|_| Error::Crypto("lock poisoned".into()))?;
        match guard.as_mut() {
            Some(vault) => vault.prepare_flush()?,
            None => None,
        }
    };
    let Some(job) = job else {
        return Ok(());
    };
    let done = job.write()?;
    if let Ok(mut guard) = state.vault.lock()
        && let Some(vault) = guard.as_mut()
    {
        vault.finish_flush(&done);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quiet_moment_ends_the_batch() {
        let (sender, receiver) = mpsc::channel();
        let started = Instant::now();
        sender.send(()).unwrap();
        assert!(wait_for_quiet(
            &receiver,
            Duration::from_millis(40),
            Duration::from_secs(5)
        ));
        let took = started.elapsed();
        assert!(
            took >= Duration::from_millis(40) && took < Duration::from_secs(1),
            "{took:?}"
        );
    }

    #[test]
    fn steady_edits_still_reach_the_disk_by_the_cap() {
        let (sender, receiver) = mpsc::channel();
        let feeder = std::thread::spawn(move || {
            for _ in 0..40 {
                if sender.send(()).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        // Like `run`: the batch starts with its first change, however late the feeder is.
        receiver.recv().unwrap();
        let started = Instant::now();
        assert!(wait_for_quiet(
            &receiver,
            Duration::from_millis(50),
            Duration::from_millis(150)
        ));
        let took = started.elapsed();
        assert!(
            took >= Duration::from_millis(150) && took < Duration::from_millis(350),
            "{took:?}"
        );
        feeder.join().unwrap();
    }

    #[test]
    fn a_closed_app_stops_waiting() {
        let (sender, receiver) = mpsc::channel::<()>();
        drop(sender);
        assert!(!wait_for_quiet(
            &receiver,
            Duration::from_millis(50),
            Duration::from_secs(1)
        ));
    }
}
