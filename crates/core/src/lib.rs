//! TradeApp `core` — pure domain, encrypted vault, pricing and simulator.
//! This crate MUST NOT depend on Tauri or any UI runtime so it stays fast
//! to compile and fully unit-testable (`cargo test -p trade-core`).

pub mod amount;
pub mod calc;
pub mod crypto;
pub mod error;
pub mod exchange;
pub mod ipc_contract;
pub mod model;
pub mod pricing;
pub mod service;
pub mod sim;
pub mod storage;
pub mod txinfo;

pub use amount::Amount;
pub use error::{Error, Result};
pub use model::*;
pub use storage::{SnapshotFormat, VaultFile};

/// Version of the encrypted *container* layout: magic, header fields, KDF
/// parameters and cipher. Independent of the payload schema — bump only when
/// the on-disk envelope changes, otherwise existing vaults stop opening.
pub const VAULT_CONTAINER_VERSION: u32 = 1;

/// Version of the plaintext *payload* schema (`VaultDoc`), stored in the
/// document's own `version` field. Bump when `VaultDoc` gains or changes a
/// field, and add a migration in `storage::decode`.
pub const VAULT_FORMAT_VERSION: u32 = 2;
/// Magic bytes prefix of a TradeApp vault file.
pub const VAULT_MAGIC: &[u8; 8] = b"TRDWVLT\x00";
