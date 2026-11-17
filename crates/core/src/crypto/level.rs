//! Vault protection level and the stages of the raise to the crypto level.

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
pub enum RaiseStage {
    Backup,
    Reencrypt,
    Verify,
}

#[cfg(test)]
mod tests {
    use crate::crypto::level;
    use crate::ipc_contract::security;

    #[test]
    fn security_types_keep_their_old_paths() {
        let security_level: level::SecurityLevel = security::SecurityLevel::Pin;
        let stage: level::RaiseStage = security::RaiseStage::Verify;
        assert_eq!(security_level, level::SecurityLevel::Pin);
        assert_eq!(stage, level::RaiseStage::Verify);
    }
}
