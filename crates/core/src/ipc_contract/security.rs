//! IPC types of the vault protection: levels, autolock, biometrics, the move
//! to the crypto level. Inputs holding a password have no `Debug`, so that one
//! cannot reach a log by accident.

use serde::{Deserialize, Serialize};

/// How the vault is protected. Never stored: it follows from the container,
/// no encryption is `Ledger`, the old `kdf_id` is `Pin`, the new one `Crypto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum SecurityLevel {
    Ledger,
    Pin,
    Crypto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum BiometricState {
    Unavailable,
    Available,
    Enabled,
    PasswordRequired,
}

/// Sleep and the lock screen always close the vault, so there is no field for them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct AutolockSettings {
    pub enabled: bool,
    pub minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SecurityState {
    pub level: SecurityLevel,
    pub min_allowed_level: SecurityLevel,
    pub has_device_keys: bool,
    pub autolock: AutolockSettings,
    pub biometric: BiometricState,
    pub pin_retry_after_secs: Option<u32>,
}

#[derive(Default, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct RaiseToCryptoInput {
    pub current_password: String,
    pub new_password: String,
    pub backup_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct RaiseToCryptoReport {
    pub level: SecurityLevel,
    pub backup_path: String,
}

/// Payload of the `security-raise-progress` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct RaiseProgress {
    pub stage: RaiseStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum RaiseStage {
    Backup,
    Reencrypt,
    Verify,
}

#[derive(Default, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct ValidatePasswordStrengthInput {
    pub password: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum PasswordStrength {
    Weak,
    Medium,
    Strong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct PasswordStrengthReport {
    pub strength: PasswordStrength,
    /// The password has the minimum length the crypto level requires.
    pub long_enough: bool,
}

#[derive(Default, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct BiometricEnableInput {
    pub password: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct BiometricUnlockInput {
    pub path: String,
}

/// What this device knows about the exchange key of a store. The key itself never
/// leaves the device file; `exchange_key_status` keeps the hint and the sub-accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeKeyState {
    /// No key on this device, even if another device imports for this store.
    Missing,
    /// Stored, but the read-only check has not gone through yet: rechecked at the next import.
    NotVerified,
    /// Stored, and Bybit confirmed it can neither trade nor withdraw.
    ReadOnly,
    /// Below the crypto level (3): no key can be added, and one still kept in
    /// the vault from before is not used.
    LevelRequired,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_state_has_the_shape_the_frontend_reads() {
        let state = SecurityState {
            level: SecurityLevel::Crypto,
            min_allowed_level: SecurityLevel::Pin,
            has_device_keys: true,
            autolock: AutolockSettings {
                enabled: true,
                minutes: 15,
            },
            biometric: BiometricState::PasswordRequired,
            pin_retry_after_secs: None,
        };
        let json = serde_json::to_value(&state).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({
                "level": "crypto",
                "min_allowed_level": "pin",
                "has_device_keys": true,
                "autolock": { "enabled": true, "minutes": 15 },
                "biometric": "password_required",
                "pin_retry_after_secs": null,
            })
        );
        let back: SecurityState = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, state);
    }

    #[test]
    fn raise_progress_names_the_stage() {
        let progress = RaiseProgress {
            stage: RaiseStage::Reencrypt,
        };
        assert_eq!(
            serde_json::to_value(progress).expect("serialize"),
            serde_json::json!({ "stage": "reencrypt" })
        );
    }
}
