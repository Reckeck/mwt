//! This device's static key for sync: other devices recognise it by the
//! public half, so the private half must survive restarts and must not be
//! readable from a copied file.
//!
//! - Android: the key is stored wrapped by an AES key in the Android Keystore
//!   (hardware-backed where the phone has it), see `crates/plugin-net`.
//! - Desktop: the OS keyring (Credential Manager, Keychain, Secret Service).
//! - Neither answering: the key lives in memory; sync works, but after a
//!   restart the devices have to pair again (the settings say so).

#[cfg(any(target_os = "android", test))]
use std::path::Path;
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
    let fails_path = dir.join("sync-device-key.fails");
    let file = std::fs::read(&path).ok();
    let unwrapped = file
        .as_deref()
        .and_then(|wrapped| crate::platform_net::unwrap_secret(app, wrapped))
        .and_then(|plain| std::str::from_utf8(&plain).ok().and_then(from_hex));
    let fails = read_fails(&fails_path);
    match key_plan(file.is_some(), unwrapped.is_some(), fails) {
        KeyPlan::Use => {
            if let Some(key) = unwrapped {
                // A good start: forget earlier failures.
                std::fs::remove_file(&fails_path).ok();
                return Ok((key, HARDWARE));
            }
        }
        KeyPlan::UseMemory => {
            // Count this start; if it cannot be counted the key is never replaced.
            write_fails(&fails_path, fails + 1);
            return Ok((DeviceKey::generate()?, MEMORY));
        }
        KeyPlan::Replace => {
            if !retire_key(&path) {
                return Ok((DeviceKey::generate()?, MEMORY));
            }
        }
        KeyPlan::Create => {}
    }
    let key = DeviceKey::generate()?;
    if let Some(wrapped) = crate::platform_net::wrap_secret(app, hex(&key.private).as_bytes()) {
        std::fs::create_dir_all(&dir).ok();
        if std::fs::write(&path, wrapped).is_ok() {
            std::fs::remove_file(&fails_path).ok();
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

/// Failed starts in a row before the stored key is given up on.
#[cfg(any(target_os = "android", test))]
const MAX_FAILED_STARTS: u32 = 3;

/// What to do with the wrapped key file on this start.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, PartialEq, Eq)]
enum KeyPlan {
    /// No file: make a key and store it.
    Create,
    /// The file unwrapped: use it.
    Use,
    /// Unwrap failed (maybe only for now): memory key, the file stays.
    UseMemory,
    /// Unwrap failed on every recent start: keep the old file aside, make a new key.
    Replace,
}

/// `failed_starts` counts earlier failed starts; this one is the next.
#[cfg(any(target_os = "android", test))]
fn key_plan(file_present: bool, unwrap_ok: bool, failed_starts: u32) -> KeyPlan {
    match (file_present, unwrap_ok) {
        (false, _) => KeyPlan::Create,
        (true, true) => KeyPlan::Use,
        (true, false) if failed_starts + 1 >= MAX_FAILED_STARTS => KeyPlan::Replace,
        (true, false) => KeyPlan::UseMemory,
    }
}

/// The counter; a missing or unreadable file counts as 0.
#[cfg(any(target_os = "android", test))]
fn read_fails(path: &Path) -> u32 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

#[cfg(any(target_os = "android", test))]
fn write_fails(path: &Path, count: u32) -> bool {
    std::fs::write(path, count.to_string()).is_ok()
}

/// Move the unusable key file to `<name>.lost` (replacing an older one);
/// `false` when that failed and the file must stay.
#[cfg(any(target_os = "android", test))]
fn retire_key(path: &Path) -> bool {
    let Some(name) = path.file_name() else {
        return false;
    };
    let mut lost = name.to_os_string();
    lost.push(".lost");
    std::fs::rename(path, path.with_file_name(lost)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_creates() {
        assert_eq!(key_plan(false, false, 0), KeyPlan::Create);
        assert_eq!(key_plan(false, false, 5), KeyPlan::Create);
    }

    #[test]
    fn good_unwrap_uses_the_file() {
        assert_eq!(key_plan(true, true, 0), KeyPlan::Use);
        assert_eq!(key_plan(true, true, 7), KeyPlan::Use);
    }

    #[test]
    fn first_two_failed_starts_use_memory_third_replaces() {
        assert_eq!(key_plan(true, false, 0), KeyPlan::UseMemory);
        assert_eq!(key_plan(true, false, 1), KeyPlan::UseMemory);
        assert_eq!(key_plan(true, false, 2), KeyPlan::Replace);
        assert_eq!(key_plan(true, false, 9), KeyPlan::Replace);
    }

    #[test]
    fn counter_defaults_to_zero_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fails");
        assert_eq!(read_fails(&path), 0);
        std::fs::write(&path, "garbage").unwrap();
        assert_eq!(read_fails(&path), 0);
        assert!(write_fails(&path, 2));
        assert_eq!(read_fails(&path), 2);
    }

    #[test]
    fn counter_that_cannot_be_written_reports_it() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!write_fails(&dir.path().join("no-dir").join("fails"), 1));
    }

    #[test]
    fn retire_keeps_the_old_file_and_overwrites_an_older_lost() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key.bin");
        std::fs::write(dir.path().join("key.bin.lost"), b"older").unwrap();
        std::fs::write(&path, b"current").unwrap();
        assert!(retire_key(&path));
        assert!(!path.exists());
        assert_eq!(
            std::fs::read(dir.path().join("key.bin.lost")).unwrap(),
            b"current"
        );
        assert!(!retire_key(&path));
    }
}
