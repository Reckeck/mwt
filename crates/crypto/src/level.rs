//! Vault protection level and the stages of the raise to the crypto level.

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
