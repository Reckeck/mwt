//! Multi Wallet Tracker `core` — pure domain, encrypted vault, pricing and market data.
//! This crate MUST NOT depend on Tauri or any UI runtime so it stays fast
//! to compile and fully unit-testable (`cargo test -p trade-core`).

pub mod amount;
pub mod app_dirs;
pub mod appconfig;
pub mod calc;
pub mod calendar;
pub mod crypto;
pub mod cryptotrack;
pub mod datasource;
pub mod error;
pub mod exchange;
pub mod external_url;
pub mod household;
pub mod ipc_contract;
pub mod jsonstore;
pub mod kernel;
pub mod limits;
pub mod market;
pub mod model;
pub mod money;
pub mod planning;
pub mod pricing;
pub mod reports;
pub mod routes;
pub mod sandbox;
pub mod secret;
pub mod service;
pub mod storage;
pub mod sync;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
pub mod time;
pub mod txinfo;
pub mod updates;
pub mod usecases;
pub mod utilities_formula;
pub mod vault_io;
pub mod workflow;

pub use amount::Amount;
pub use error::{Error, Result};
pub use model::*;
pub use storage::{SnapshotFormat, VaultFile};

/// Version of the encrypted *container* layout: magic, header fields, KDF
/// parameters and cipher, and the one number that tells an old payload from a
/// new one.
///
/// 1: the files of every build before the format reset of 2026-11-19; the
/// payload `version` there counts 1 to 4. 2: every file written since; the
/// payload `version` counts again from 1. The reader accepts both (container 1
/// only for the one-time transition of a payload v4, see
/// `storage::decode_file`); every write is container 2. A build that
/// predates the reset refuses a container 2 file at the header, so it cannot
/// misread a new payload v1 as an old one.
pub const VAULT_CONTAINER_VERSION: u32 = 2;

/// The container version of the files before the format reset; read, never
/// written.
pub const VAULT_LEGACY_CONTAINER_VERSION: u32 = 1;

/// Version of the plaintext *payload* schema (`VaultDoc`), stored in the
/// document's own `version` field. It is read together with the container
/// version: payload 1 in container 2 is this format.
///
/// The payload carries field names (JSON or MessagePack), so a field added
/// with `#[serde(default)]` needs no bump. Bump only for a change older files
/// cannot be read with (a renamed or re-typed field, a removed variant): freeze
/// the current shape in a module of its own, convert it into the new document
/// on open after the untouched file is copied to `<vault>.v<N>.bak`, and test
/// it with a committed file. A bump without a transition is never acceptable;
/// a version from the future is refused.
///
/// Note this is unrelated to [`VAULT_CONTAINER_VERSION`], which versions the
/// encrypted envelope and only moves when the on-disk bytes change.
pub const VAULT_FORMAT_VERSION: u32 = 1;
/// Magic bytes prefix of a vault file. Kept from the TradeApp days: changing
/// them would make every existing vault unreadable.
pub const VAULT_MAGIC: &[u8; 8] = b"TRDWVLT\x00";
