//! Multi Wallet Tracker `core` — pure domain, encrypted vault, pricing and market data.
//! This crate MUST NOT depend on Tauri or any UI runtime so it stays fast
//! to compile and fully unit-testable (`cargo test -p trade-core`).

pub mod amount;
pub mod app_dirs;
pub mod appconfig;
pub mod calc;
pub mod calendar;
pub mod crypto;
pub mod datasource;
pub mod error;
pub mod exchange;
pub mod ipc_contract;
pub mod market;
pub mod model;
pub mod pricing;
pub mod routes;
pub mod sandbox;
pub mod service;
pub mod storage;
pub mod sync;
pub mod txinfo;
pub mod utilities_formula;
pub mod vault_io;
pub mod workflow;

pub use amount::Amount;
pub use error::{Error, Result};
pub use model::*;
pub use storage::{SnapshotFormat, VaultFile};

/// Version of the encrypted *container* layout: magic, header fields, KDF
/// parameters and cipher. Independent of the payload schema — bump only when
/// the on-disk envelope changes, otherwise existing vaults stop opening.
pub const VAULT_CONTAINER_VERSION: u32 = 1;

/// Version of the plaintext *payload* schema (`VaultDoc`), stored in the
/// document's own `version` field.
///
/// Older versions are read and migrated on open (`storage::decode_versioned`),
/// after the untouched file is copied to `<vault>.v<N>.bak`; a version from the
/// future is refused. From v2 the payload carries field names (JSON or
/// MessagePack), so a field added with `#[serde(default)]` needs no bump.
///
/// Bump only for a change older files cannot be read with (a renamed or
/// re-typed field, a removed variant): freeze the current shape in
/// `storage::vN`, add the migration step and tests for both formats. The user
/// allowed bumps with a migration on 2026-10-11 and prefers them to hacks
/// around a broken format; a bump without a migration is never acceptable.
///
/// Note this is unrelated to [`VAULT_CONTAINER_VERSION`], which versions the
/// encrypted envelope and only moves when the on-disk bytes change.
pub const VAULT_FORMAT_VERSION: u32 = 3;
/// Magic bytes prefix of a vault file. Kept from the TradeApp days: changing
/// them would make every existing vault unreadable.
pub const VAULT_MAGIC: &[u8; 8] = b"TRDWVLT\x00";
