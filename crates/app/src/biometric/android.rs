//! Android: an AES-256-GCM key in the AndroidKeyStore (StrongBox when the phone has
//! one) that works only after a `BiometricPrompt` gesture, through a `CryptoObject`
//! (`crates/plugin-biometric`). The key requires strong biometrics for every use and
//! is dropped by the system when a fingerprint is added or the biometrics are reset.
//! The record holds `iv || ciphertext` of the vault password; the vault id is bound
//! as AEAD data by the alias. Unverified on a real device: see the H7B report.
//!
//! Threads: every plugin call blocks the calling thread until the Kotlin side
//! answers, and the Kotlin command itself runs on the Android main thread. So a call
//! must never come from the main thread, and never with a lock held that the main
//! thread may wait for. The commands call `available`/`wrap`/`unwrap` from tokio
//! workers without locks (`wrap`/`unwrap` through `spawn_blocking`). `invalidate` is
//! also reached with the vault mutex held (password change), so it does not wait:
//! the delete runs on a short thread, and the next `wrap`/`unwrap` joins it first so
//! a late delete can never hit a freshly made key.

use std::sync::Mutex;
use std::thread::JoinHandle;

use sha2::{Digest, Sha256};
use tauri_plugin_biometric::Status;
use zeroize::Zeroizing;

use super::{BioError, BiometricVault};

#[derive(Default)]
pub struct Platform;

/// Key deletions that were started by `invalidate` and may still be running.
static PENDING_DELETES: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

/// Wait for the deletions started earlier (called before a call that can create or use a key).
fn settle_deletes() {
    let pending = PENDING_DELETES
        .lock()
        .map(|mut list| std::mem::take(&mut *list))
        .unwrap_or_default();
    for handle in pending {
        handle.join().ok();
    }
}

/// The Keystore alias: a hash of the vault id, so a path never becomes part of a name.
fn alias(vault_id: &str) -> String {
    let digest = Sha256::digest(vault_id.as_bytes());
    let hex: String = digest
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("mwt-bio-{hex}")
}

fn map_status(status: Status) -> BioError {
    match status {
        Status::Cancelled => BioError::Cancelled,
        Status::Invalidated => BioError::Invalidated,
        Status::Unavailable => BioError::Unavailable,
        Status::Ok | Status::Failed => BioError::Failed,
    }
}

impl BiometricVault for Platform {
    fn available(&self) -> bool {
        tauri_plugin_biometric::available().unwrap_or(false)
    }

    fn wrap(&self, vault_id: &str, secret: &[u8]) -> Result<Vec<u8>, BioError> {
        settle_deletes();
        let name = alias(vault_id);
        let (status, blob) =
            tauri_plugin_biometric::encrypt(&name, secret).map_err(|_| BioError::Failed)?;
        if status == Status::Ok {
            return Ok(blob);
        }
        // A key made for a prompt that did not finish must not stay around.
        self.invalidate(vault_id).ok();
        Err(match map_status(status) {
            // A key that cannot be made is not a stale binding.
            BioError::Invalidated => BioError::Failed,
            other => other,
        })
    }

    fn unwrap(&self, vault_id: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, BioError> {
        settle_deletes();
        let (status, secret) = tauri_plugin_biometric::decrypt(&alias(vault_id), blob)
            .map_err(|_| BioError::Failed)?;
        let secret = Zeroizing::new(secret);
        if status == Status::Ok {
            Ok(secret)
        } else {
            Err(map_status(status))
        }
    }

    fn invalidate(&self, vault_id: &str) -> Result<(), BioError> {
        // A missing key is already what is wanted. Does not wait for the main thread.
        let name = alias(vault_id);
        let handle = std::thread::spawn(move || {
            tauri_plugin_biometric::delete_key(&name).ok();
        });
        if let Ok(mut list) = PENDING_DELETES.lock() {
            list.push(handle);
        }
        Ok(())
    }
}
