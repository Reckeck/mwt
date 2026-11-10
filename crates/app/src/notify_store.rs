//! The notification snapshot kept for a locked vault (plan §14, option B).
//!
//! It lives in memory, and on disk only encrypted (XChaCha20-Poly1305) with a
//! random key kept in the OS keyring (Secret Service, Keychain, Credential
//! Manager). Without a keyring it stays in memory: reminders still come while
//! the app runs, just not after a restart. Never written in the clear.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use tauri::{AppHandle, Manager};

use trade_core::model::notify::NoticeRecord;
use trade_core::service::{AlertBase, NotificationSnapshot};

/// Where the key lived while the app was TradeApp; moved on first use.
#[cfg(desktop)]
const LEGACY_KEYRING_SERVICE: &str = crate::legacy::LEGACY_IDENTIFIER;
#[cfg(desktop)]
const KEYRING_USER: &str = "notification-snapshot";
const FILE_NAME: &str = "notify-snapshot.bin";
const NONCE_LEN: usize = 24;

/// What the scheduler keeps between ticks for notices while locked.
#[derive(Default)]
pub struct NotifyMemory {
    pub snapshot: Option<NotificationSnapshot>,
    /// Shown while the vault was locked; merged into its log at unlock.
    pub shown: Vec<NoticeRecord>,
    /// Alerts that fired while locked: `(alert_id, when)`.
    pub fired_alerts: Vec<(String, String)>,
    /// What each fired alert looked like when it was checked, by alert id.
    pub fired_bases: HashMap<String, AlertBase>,
    /// Whether the snapshot can be kept on disk (a keyring answered).
    pub persistent: bool,
    pub last_snapshot: Option<Instant>,
    pub last_alert_check: Option<Instant>,
}

/// Every field defaults: a snapshot written by another build is missing
/// whatever was added since, and losing one collection must not throw the
/// rest of the file away.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Stored {
    snapshot: Option<NotificationSnapshot>,
    shown: Vec<NoticeRecord>,
    fired_alerts: Vec<(String, String)>,
    /// Absent in a file of an older build: those firings apply as before.
    fired_bases: HashMap<String, AlertBase>,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(FILE_NAME))
}

#[cfg(desktop)]
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(desktop)]
fn unhex(text: &str) -> Option<[u8; 32]> {
    let mut out = [0u8; 32];
    if text.len() != 64 {
        return None;
    }
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// The snapshot key from the keyring, made on first use. `None` without one.
#[cfg(desktop)]
fn key() -> Option<[u8; 32]> {
    let entry = keyring::Entry::new(crate::app_id::get(), KEYRING_USER).ok()?;
    match entry.get_password() {
        Ok(stored) => unhex(&stored),
        Err(keyring::Error::NoEntry) => {
            if let Some(stored) = take_legacy_key() {
                entry.set_password(&stored).ok()?;
                return unhex(&stored);
            }
            let fresh: [u8; 32] = rand::random();
            entry.set_password(&hex(&fresh)).ok()?;
            Some(fresh)
        }
        Err(_) => None,
    }
}

/// The key kept under the TradeApp service name, removed once read. It
/// belongs to the dev channel, like the rest of the TradeApp data.
#[cfg(desktop)]
fn take_legacy_key() -> Option<String> {
    if crate::app_id::get() != crate::app_id::DEV {
        return None;
    }
    let legacy = keyring::Entry::new(LEGACY_KEYRING_SERVICE, KEYRING_USER).ok()?;
    let stored = legacy.get_password().ok()?;
    legacy.delete_credential().ok();
    Some(stored)
}

/// No OS keyring on mobile: the snapshot stays in memory.
#[cfg(mobile)]
fn key() -> Option<[u8; 32]> {
    None
}

/// Read what was kept on disk, if the keyring opens it.
pub fn load(app: &AppHandle) -> NotifyMemory {
    let Some(key) = key() else {
        return NotifyMemory::default();
    };
    let mut memory = NotifyMemory {
        persistent: true,
        ..NotifyMemory::default()
    };
    let Some(bytes) = file(app).and_then(|path| std::fs::read(path).ok()) else {
        return memory;
    };
    if bytes.len() <= NONCE_LEN {
        return memory;
    }
    let (nonce, sealed) = bytes.split_at(NONCE_LEN);
    let Ok(cipher) = XChaCha20Poly1305::new_from_slice(&key) else {
        return memory;
    };
    let Ok(plain) = cipher.decrypt(XNonce::from_slice(nonce), sealed) else {
        return memory;
    };
    match serde_json::from_slice::<Stored>(&plain) {
        Ok(stored) => {
            memory.snapshot = stored.snapshot;
            memory.shown = stored.shown;
            memory.fired_alerts = stored.fired_alerts;
            memory.fired_bases = stored.fired_bases;
        }
        // It decrypted, so the keyring key is right and the file is ours:
        // nothing to retry, but a silent empty snapshot would look like a
        // first start for ever.
        Err(err) => eprintln!("notifications: the kept snapshot is unreadable: {err}"),
    }
    memory
}

/// Keep the memory on disk, encrypted; nothing is written without a keyring.
pub fn save(app: &AppHandle, memory: &mut NotifyMemory) {
    let Some(key) = key() else {
        memory.persistent = false;
        return;
    };
    memory.persistent = true;
    let Some(path) = file(app) else { return };
    let stored = Stored {
        snapshot: memory.snapshot.clone(),
        shown: memory.shown.clone(),
        fired_alerts: memory.fired_alerts.clone(),
        fired_bases: memory.fired_bases.clone(),
    };
    let Ok(plain) = serde_json::to_vec(&stored) else {
        return;
    };
    let Ok(cipher) = XChaCha20Poly1305::new_from_slice(&key) else {
        return;
    };
    let nonce: [u8; NONCE_LEN] = rand::random();
    let Ok(sealed) = cipher.encrypt(XNonce::from_slice(&nonce), plain.as_slice()) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut bytes = nonce.to_vec();
    bytes.extend_from_slice(&sealed);
    let _ = trade_core::crypto::atomic_write(&path, &bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A snapshot file of a build that did not have a field yet still loads,
    /// with that field at its default.
    #[test]
    fn a_snapshot_from_an_older_build_loads_with_defaults() {
        let stored: Stored = serde_json::from_slice(b"{}").expect("a file from before the fields");
        assert!(stored.snapshot.is_none());
        assert!(stored.shown.is_empty());
        assert!(stored.fired_alerts.is_empty());
        assert!(stored.fired_bases.is_empty());

        let partial: Stored = serde_json::from_slice(br#"{"shown":[]}"#).expect("older file");
        assert!(partial.fired_alerts.is_empty());
    }

    #[test]
    fn a_key_survives_hex() {
        let key: [u8; 32] = rand::random();
        assert_eq!(unhex(&hex(&key)), Some(key));
        assert_eq!(unhex("zz"), None);
    }
}
