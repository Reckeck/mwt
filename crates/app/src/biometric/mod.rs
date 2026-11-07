//! Quick unlock of a level-3 vault with the device biometrics (H7A).
//!
//! The vault password is bound to the device by the OS: it is released only
//! after a biometric gesture that the OS itself enforces. A platform without
//! such a binding reports `Unavailable`; a prompt shown by this code without a
//! bound secret behind it is never an option.
//!
//! Layers: `BiometricVault` is the platform seam (`macos.rs`; `windows.rs`
//! drops in later), `Gate` adds what is the same everywhere (the device record
//! with the time of the last password entry, the 7-day rule, the reset), and
//! `Clock` makes the rule testable. The record lives in the app config dir, next
//! to the app settings: not in the vault and never synced. The core knows
//! nothing about any of this.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "macos", windows)))]
mod unsupported;
#[cfg(windows)]
mod windows;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use trade_core::ipc_contract::security::BiometricState;
use zeroize::Zeroizing;

#[cfg(target_os = "macos")]
pub use macos::Platform;
#[cfg(not(any(target_os = "macos", windows)))]
pub use unsupported::Platform;
#[cfg(windows)]
pub use windows::Platform;

/// A password typed longer ago than this is asked for again.
pub const PASSWORD_WINDOW_SECS: u64 = 7 * 24 * 60 * 60;

const RECORD_FILE: &str = "biometric.json";
/// Serializes the read-modify-write of the record file between commands.
static RECORD_LOCK: Mutex<()> = Mutex::new(());

/// Why a biometric operation did not give a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BioError {
    /// No hardware-bound storage here, or biometrics are not set up.
    Unavailable,
    /// Quick unlock is not turned on for this vault.
    NotEnabled,
    /// The last password entry is older than the window.
    PasswordRequired,
    /// The OS dropped the binding (new fingerprint, reset): the record is gone.
    Invalidated,
    /// The user dismissed the system prompt or it did not recognise them.
    // Only a platform with a real prompt reports it.
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
    Cancelled,
    Failed,
}

impl BioError {
    /// Code of the `"CODE: message"` string the frontend parses.
    pub fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "BIOMETRIC_UNAVAILABLE",
            Self::NotEnabled => "BIOMETRIC_NOT_ENABLED",
            Self::PasswordRequired => "BIOMETRIC_PASSWORD_REQUIRED",
            Self::Invalidated => "BIOMETRIC_INVALIDATED",
            Self::Cancelled => "BIOMETRIC_CANCELLED",
            Self::Failed => "BIOMETRIC_FAILED",
        }
    }

    pub fn to_ipc(self) -> String {
        format!("{}: biometric unlock: {:?}", self.code(), self)
    }
}

/// The platform seam: a secret that the OS releases only after a biometric
/// gesture. `wrap` returns the blob the device record keeps (empty when the OS
/// keeps the secret itself).
pub trait BiometricVault {
    /// Hardware-bound storage and enrolled biometrics are there.
    fn available(&self) -> bool;
    fn wrap(&self, vault_id: &str, secret: &[u8]) -> Result<Vec<u8>, BioError>;
    /// Gesture inside; `Invalidated` when the OS dropped the binding.
    fn unwrap(&self, vault_id: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, BioError>;
    /// Remove the binding; removing a missing one succeeds.
    fn invalidate(&self, vault_id: &str) -> Result<(), BioError>;
}

/// Unix seconds; a fake one drives the 7-day tests.
pub trait Clock {
    fn now(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Records {
    vaults: BTreeMap<String, Record>,
}

#[derive(Serialize, Deserialize)]
struct Record {
    /// When the password was last typed for this vault.
    last_password_at: u64,
    blob: Vec<u8>,
}

pub struct Gate<B, C> {
    backend: B,
    clock: C,
    file: PathBuf,
}

/// The gate the commands use: the platform backend, the system clock, the app config dir.
pub fn system_gate(dir: &Path) -> Gate<Platform, SystemClock> {
    Gate::new(Platform, SystemClock, dir)
}

impl<B: BiometricVault, C: Clock> Gate<B, C> {
    pub fn new(backend: B, clock: C, dir: &Path) -> Self {
        Self {
            backend,
            clock,
            file: dir.join(RECORD_FILE),
        }
    }

    pub fn state(&self, vault_id: &str) -> BiometricState {
        if !self.backend.available() {
            return BiometricState::Unavailable;
        }
        match self.record(vault_id) {
            None => BiometricState::Available,
            Some(record) if self.expired(&record) => BiometricState::PasswordRequired,
            Some(_) => BiometricState::Enabled,
        }
    }

    /// Bind `secret` (the vault password, just verified) and start the window.
    pub fn enable(&self, vault_id: &str, secret: &[u8]) -> Result<(), BioError> {
        if !self.backend.available() {
            return Err(BioError::Unavailable);
        }
        let blob = self.backend.wrap(vault_id, secret)?;
        let record = Record {
            last_password_at: self.clock.now(),
            blob,
        };
        if self.store(vault_id, Some(record)).is_err() {
            self.backend.invalidate(vault_id).ok();
            return Err(BioError::Failed);
        }
        Ok(())
    }

    /// Release the password, after the OS gesture and only inside the window.
    pub fn unlock(&self, vault_id: &str) -> Result<Zeroizing<Vec<u8>>, BioError> {
        let record = self.record(vault_id).ok_or(BioError::NotEnabled)?;
        if self.expired(&record) {
            return Err(BioError::PasswordRequired);
        }
        match self.backend.unwrap(vault_id, &record.blob) {
            Err(BioError::Invalidated) => {
                self.disable(vault_id);
                Err(BioError::Invalidated)
            }
            other => other,
        }
    }

    /// The password was typed: the window starts over. Nothing happens when quick unlock is off.
    pub fn password_entered(&self, vault_id: &str) {
        if let Some(mut record) = self.record(vault_id) {
            record.last_password_at = self.clock.now();
            self.store(vault_id, Some(record)).ok();
        }
    }

    /// Turn it off or reset it (password change, user choice): binding and record go.
    pub fn disable(&self, vault_id: &str) {
        self.backend.invalidate(vault_id).ok();
        self.store(vault_id, None).ok();
    }

    /// A clock that went back counts as expired: the window cannot be stretched by it.
    fn expired(&self, record: &Record) -> bool {
        let now = self.clock.now();
        now < record.last_password_at || now - record.last_password_at > PASSWORD_WINDOW_SECS
    }

    fn record(&self, vault_id: &str) -> Option<Record> {
        let _guard = RECORD_LOCK.lock().ok()?;
        self.read().vaults.remove(vault_id)
    }

    fn read(&self) -> Records {
        std::fs::read(&self.file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Put (or, with `None`, drop) the record of one vault; the file is replaced atomically.
    fn store(&self, vault_id: &str, record: Option<Record>) -> std::io::Result<()> {
        let _guard = RECORD_LOCK
            .lock()
            .map_err(|_| std::io::Error::other("lock poisoned"))?;
        let mut records = self.read();
        match record {
            Some(record) => records.vaults.insert(vault_id.to_string(), record),
            None => records.vaults.remove(vault_id),
        };
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(&records)?)?;
        std::fs::rename(&tmp, &self.file)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::*;

    const DAY: u64 = 24 * 60 * 60;

    #[derive(Clone, Default)]
    struct FakeClock(Rc<Cell<u64>>);

    impl Clock for FakeClock {
        fn now(&self) -> u64 {
            self.0.get()
        }
    }

    /// Keeps the secrets in memory; `bound = false` plays a changed fingerprint set.
    #[derive(Clone)]
    struct FakeBackend {
        available: bool,
        bound: Rc<Cell<bool>>,
        secrets: Rc<RefCell<BTreeMap<String, Vec<u8>>>>,
    }

    impl FakeBackend {
        fn new(available: bool) -> Self {
            Self {
                available,
                bound: Rc::new(Cell::new(true)),
                secrets: Rc::default(),
            }
        }
    }

    impl BiometricVault for FakeBackend {
        fn available(&self) -> bool {
            self.available
        }

        fn wrap(&self, vault_id: &str, secret: &[u8]) -> Result<Vec<u8>, BioError> {
            self.secrets
                .borrow_mut()
                .insert(vault_id.to_string(), secret.to_vec());
            Ok(Vec::new())
        }

        fn unwrap(&self, vault_id: &str, _blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, BioError> {
            if !self.bound.get() {
                return Err(BioError::Invalidated);
            }
            self.secrets
                .borrow()
                .get(vault_id)
                .map(|secret| Zeroizing::new(secret.clone()))
                .ok_or(BioError::Invalidated)
        }

        fn invalidate(&self, vault_id: &str) -> Result<(), BioError> {
            self.secrets.borrow_mut().remove(vault_id);
            Ok(())
        }
    }

    fn gate(
        available: bool,
    ) -> (
        Gate<FakeBackend, FakeClock>,
        FakeBackend,
        FakeClock,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().expect("tempdir");
        let backend = FakeBackend::new(available);
        let clock = FakeClock::default();
        clock.0.set(1_000_000);
        let gate = Gate::new(backend.clone(), clock.clone(), dir.path());
        (gate, backend, clock, dir)
    }

    #[test]
    fn an_unavailable_platform_has_no_button_and_refuses_to_enable() {
        let (gate, _backend, _clock, _dir) = gate(false);
        assert_eq!(gate.state("a"), BiometricState::Unavailable);
        assert_eq!(gate.enable("a", b"pw"), Err(BioError::Unavailable));
        assert_eq!(gate.unlock("a").unwrap_err(), BioError::NotEnabled);
    }

    #[test]
    fn enable_then_unlock_gives_the_password_back() {
        let (gate, _backend, _clock, _dir) = gate(true);
        assert_eq!(gate.state("a"), BiometricState::Available);
        gate.enable("a", b"correct horse").expect("enable");
        assert_eq!(gate.state("a"), BiometricState::Enabled);
        assert_eq!(
            gate.unlock("a").expect("unlock").as_slice(),
            b"correct horse"
        );
    }

    #[test]
    fn the_password_is_asked_for_after_seven_days() {
        let (gate, _backend, clock, _dir) = gate(true);
        gate.enable("a", b"pw").expect("enable");
        let start = clock.0.get();
        clock.0.set(start + 7 * DAY);
        assert!(gate.unlock("a").is_ok(), "exactly 7 days is still inside");
        clock.0.set(start + 7 * DAY + 1);
        assert_eq!(gate.unlock("a").unwrap_err(), BioError::PasswordRequired);
        assert_eq!(gate.state("a"), BiometricState::PasswordRequired);
    }

    #[test]
    fn typing_the_password_starts_the_window_over() {
        let (gate, _backend, clock, _dir) = gate(true);
        gate.enable("a", b"pw").expect("enable");
        let start = clock.0.get();
        clock.0.set(start + 6 * DAY);
        gate.password_entered("a");
        clock.0.set(start + 12 * DAY);
        assert!(gate.unlock("a").is_ok());
        assert_eq!(gate.state("a"), BiometricState::Enabled);
    }

    #[test]
    fn a_clock_that_went_back_does_not_stretch_the_window() {
        let (gate, _backend, clock, _dir) = gate(true);
        gate.enable("a", b"pw").expect("enable");
        clock.0.set(clock.0.get() - 1);
        assert_eq!(gate.unlock("a").unwrap_err(), BioError::PasswordRequired);
    }

    #[test]
    fn an_invalidated_binding_removes_the_record() {
        let (gate, backend, _clock, _dir) = gate(true);
        gate.enable("a", b"pw").expect("enable");
        backend.bound.set(false);
        assert_eq!(gate.unlock("a").unwrap_err(), BioError::Invalidated);
        assert_eq!(gate.state("a"), BiometricState::Available);
        assert_eq!(gate.unlock("a").unwrap_err(), BioError::NotEnabled);
    }

    #[test]
    fn disable_removes_binding_and_record() {
        let (gate, backend, _clock, _dir) = gate(true);
        gate.enable("a", b"pw").expect("enable");
        gate.disable("a");
        assert_eq!(gate.state("a"), BiometricState::Available);
        assert!(backend.secrets.borrow().is_empty());
    }

    #[test]
    fn vaults_do_not_share_records_and_the_record_survives_a_new_gate() {
        let (gate, backend, clock, dir) = gate(true);
        gate.enable("a", b"pw").expect("enable");
        assert_eq!(gate.state("b"), BiometricState::Available);
        let again = Gate::new(backend, clock, dir.path());
        assert_eq!(again.state("a"), BiometricState::Enabled);
    }
}
