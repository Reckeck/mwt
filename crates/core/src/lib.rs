//! Multi Wallet Tracker `core` — pure domain, encrypted vault, pricing and market data.
//! This crate MUST NOT depend on Tauri or any UI runtime so it stays fast
//! to compile and fully unit-testable (`cargo test -p trade-core`).

pub mod app_dirs;
pub mod appconfig;
pub mod crypto;
pub mod cryptotrack;
pub mod datasource;
pub mod exchange;
pub mod external_url;
pub mod household;
pub mod ipc_contract;
pub mod jsonstore;
pub mod kernel;
pub mod market;
pub mod model;
pub mod money;
pub mod planning;
pub mod pricing;
pub mod reports;
pub mod sandbox;
pub mod service;
pub mod storage;
pub mod sync;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
pub mod txinfo;
pub mod updates;
pub mod usecases;
pub mod vault_io;
pub mod workflow;

pub use model::*;
pub use mwt_base::{
    Amount, Error, Result, amount, calc, calendar, error, limits, routes, secret, time,
    utilities_formula,
};
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
#[cfg(test)]
mod base_facade_tests {
    //! The paths of `mwt-base` that other modules of the facade still reach: the
    //! old homes of moved items keep resolving to the same functions and values.

    use chrono::NaiveDate;

    use crate::pricing::history;
    use crate::{limits, model, service, time};

    #[test]
    fn limits_keep_their_old_paths() {
        assert_eq!(
            service::security_level::MIN_PIN_DIGITS,
            limits::MIN_PIN_DIGITS
        );
        assert_eq!(model::store::MAX_STORE_DEPTH, limits::MAX_STORE_DEPTH);
    }

    #[test]
    fn moved_time_helpers_keep_their_old_paths() {
        let day = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let iso = "2026-03-01T20:00:00Z";
        assert_eq!(history::unix_secs(iso), time::unix_secs(iso));
        assert_eq!(history::secs_or_now(iso), time::secs_or_now(iso));
        assert_eq!(
            crate::cryptotrack::prices::date_secs(day),
            time::date_secs(day)
        );
        assert_eq!(crate::cryptotrack::prices::date_ts(day), time::date_ts(day));
    }
}
