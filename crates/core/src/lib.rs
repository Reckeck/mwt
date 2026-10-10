//! TradeApp `core` — pure domain, encrypted vault, pricing and market data.
//! This crate MUST NOT depend on Tauri or any UI runtime so it stays fast
//! to compile and fully unit-testable (`cargo test -p trade-core`).

pub mod amount;
pub mod appconfig;
pub mod calc;
pub mod calendar;
pub mod crypto;
pub mod error;
pub mod exchange;
pub mod ipc_contract;
pub mod market;
pub mod model;
pub mod pricing;
pub mod service;
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
/// document's own `version` field.
///
/// # DO NOT BUMP THIS WITHOUT THE USER'S EXPLICIT PERMISSION
///
/// This constant is deliberately pinned at `1`. Previous versions of this
/// codebase bumped it every time `VaultDoc` gained a field, which meant every
/// schema tweak silently locked users out of their own vault: `storage::decode`
/// rejects any `version` it does not recognize, and there was no migration to
/// fall back on. The user has decided that schema changes ship without a
/// version bump — additive fields are handled with `#[serde(default)]` instead
/// (see the `vault-crypto` skill).
///
/// If you believe a bump is genuinely unavoidable — a change that cannot be
/// made backward compatible — stop and ask the user first. Do not edit this
/// number on your own initiative.
///
/// Note this is unrelated to [`VAULT_CONTAINER_VERSION`], which versions the
/// encrypted envelope and only moves when the on-disk bytes change.
pub const VAULT_FORMAT_VERSION: u32 = 1;
/// Magic bytes prefix of a TradeApp vault file.
pub const VAULT_MAGIC: &[u8; 8] = b"TRDWVLT\x00";
