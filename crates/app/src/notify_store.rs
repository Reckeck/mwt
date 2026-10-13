//! The notification snapshot kept for a locked vault (plan §14, option Б).
//!
//! It lives in memory, and on disk only encrypted (XChaCha20-Poly1305) with a
//! random key kept in the OS keyring (Secret Service, Keychain, Credential
//! Manager). Without a keyring it stays in memory: reminders still come while
//! the app runs, just not after a restart. Never written in the clear.

use std::path::PathBuf;
use std::time::Instant;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use tauri::{AppHandle, Manager};

use trade_core::model::notify::NoticeRecord;
use trade_core::service::NotificationSnapshot;

#[cfg(desktop)]
const KEYRING_SERVICE: &str = "dev.tradeapp.desktop";
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
    /// Whether the snapshot can be kept on disk (a keyring answered).
    pub persistent: bool,
    pub last_snapshot: Option<Instant>,
    pub last_alert_check: Option<Instant>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Stored {
    snapshot: Option<NotificationSnapshot>,
    shown: Vec<NoticeRecord>,
    fired_alerts: Vec<(String, String)>,
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
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
    match entry.get_password() {
        Ok(stored) => unhex(&stored),
        Err(keyring::Error::NoEntry) => {
            let fresh: [u8; 32] = rand::random();
            entry.set_password(&hex(&fresh)).ok()?;
            Some(fresh)
        }
        Err(_) => None,
    }
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
    if let Ok(stored) = serde_json::from_slice::<Stored>(&plain) {
        memory.snapshot = stored.snapshot;
        memory.shown = stored.shown;
        memory.fired_alerts = stored.fired_alerts;
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

    #[test]
    fn a_key_survives_hex() {
        let key: [u8; 32] = rand::random();
        assert_eq!(unhex(&hex(&key)), Some(key));
        assert_eq!(unhex("zz"), None);
    }
}
