//! Vault protection level, the stages of the raise to the crypto level, the strength of a
//! password and the state of an exchange key on this device.

use serde::{Deserialize, Serialize};

/// How the vault is protected. Never stored: it follows from the container,
/// no encryption is `Ledger`, the old `kdf_id` is `Pin`, the new one `Crypto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SecurityLevel {
    Ledger,
    Pin,
    Crypto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RaiseStage {
    Backup,
    Reencrypt,
    Verify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PasswordStrength {
    Weak,
    Medium,
    Strong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PasswordStrengthReport {
    pub strength: PasswordStrength,
    /// The password has the minimum length the crypto level requires.
    pub long_enough: bool,
}

/// What this device knows about the exchange key of a store. The key itself never
/// leaves the device file; `exchange_key_status` keeps the hint and the sub-accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
