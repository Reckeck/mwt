//! This device's static key for sync: other devices recognise it by the
//! public half, so the private half must survive restarts and must not be
//! readable from a copied file.
//!
//! - Android: the key is stored wrapped by an AES key in the Android Keystore
//!   (hardware-backed where the phone has it), see `crates/plugin-net`.
//! - Desktop: the OS keyring (Credential Manager, Keychain, Secret Service).
//! - Neither answering: the key lives in memory; sync works, but after a
//!   restart the devices have to pair again (the settings say so).

use std::sync::Mutex;

use tauri::AppHandle;
use trade_core::error::Result;
use trade_core::sync::session::{DeviceKey, hex, unhex};

/// Where the key lives, as `SyncStatus::key_storage` reports it.
#[cfg(target_os = "android")]
pub const HARDWARE: &str = "hardware";
#[cfg(desktop)]
pub const SYSTEM: &str = "system";
pub const MEMORY: &str = "memory";

#[cfg(desktop)]
const KEYRING_USER: &str = "sync-device-key";

static CACHE: Mutex<Option<(DeviceKey, &'static str)>> = Mutex::new(None);

/// The device key and where it is kept; made on first use.
pub fn device_key(app: &AppHandle) -> Result<(DeviceKey, &'static str)> {
    if let Ok(guard) = CACHE.lock()
        && let Some(cached) = guard.as_ref()
    {
        return Ok(cached.clone());
    }
    let found = load_or_create(app)?;
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(found.clone());
    }
    Ok(found)
}

/// Where the key is (or would be) kept, without making one.
pub fn storage(app: &AppHandle) -> &'static str {
    device_key(app)
        .map(|(_, storage)| storage)
        .unwrap_or(MEMORY)
}

fn from_hex(stored: &str) -> Option<DeviceKey> {
    DeviceKey::from_private(&unhex(stored.trim())?).ok()
}

#[cfg(desktop)]
fn load_or_create(_app: &AppHandle) -> Result<(DeviceKey, &'static str)> {
    if let Ok(entry) = keyring::Entry::new(crate::app_id::get(), KEYRING_USER) {
        match entry.get_password() {
            Ok(stored) => {
                if let Some(key) = from_hex(&stored) {
                    return Ok((key, SYSTEM));
                }
            }
            Err(keyring::Error::NoEntry) => {
                let key = DeviceKey::generate()?;
                if entry.set_password(&hex(&key.private)).is_ok() {
                    return Ok((key, SYSTEM));
                }
            }
            Err(_) => {}
        }
    }
    Ok((DeviceKey::generate()?, MEMORY))
}

#[cfg(target_os = "android")]
fn load_or_create(app: &AppHandle) -> Result<(DeviceKey, &'static str)> {
    use tauri::Manager;
    let Ok(dir) = app.path().app_data_dir() else {
        return Ok((DeviceKey::generate()?, MEMORY));
    };
    let path = dir.join("sync-device-key.bin");
    if let Ok(wrapped) = std::fs::read(&path)
        && let Some(plain) = crate::platform_net::unwrap_secret(app, &wrapped)
        && let Some(key) = std::str::from_utf8(&plain).ok().and_then(from_hex)
    {
        return Ok((key, HARDWARE));
    }
    let key = DeviceKey::generate()?;
    if let Some(wrapped) = crate::platform_net::wrap_secret(app, hex(&key.private).as_bytes()) {
        std::fs::create_dir_all(&dir).ok();
        if std::fs::write(&path, wrapped).is_ok() {
            return Ok((key, HARDWARE));
        }
    }
    Ok((key, MEMORY))
}

/// iOS has no plugin yet: the key lives in memory.
#[cfg(all(mobile, not(target_os = "android")))]
fn load_or_create(_app: &AppHandle) -> Result<(DeviceKey, &'static str)> {
    Ok((DeviceKey::generate()?, MEMORY))
}
