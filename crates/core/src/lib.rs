//! Multi Wallet Tracker `core` — pure domain, encrypted vault, pricing and market data.
//! This crate MUST NOT depend on Tauri or any UI runtime so it stays fast
//! to compile and fully unit-testable (`cargo test -p trade-core`).

pub mod app_dirs;
pub mod appconfig;
pub mod cryptotrack;
pub mod datasource;
pub mod exchange;
pub mod external_url;
#[cfg(test)]
mod household_facade_tests;
pub mod ipc_contract;
pub mod jsonstore;
pub mod kernel;
pub mod market;
pub mod model;
#[cfg(test)]
mod money_facade_tests;
#[cfg(test)]
mod planning_facade_tests;
pub mod pricing;
pub mod reports;
#[cfg(test)]
mod reports_facade_tests;
pub mod sandbox;
pub mod service;
pub mod storage;
#[cfg(test)]
mod sync_facade_tests;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
pub mod txinfo;
pub mod updates;
pub mod usecases;
#[cfg(test)]
mod vault_facade_tests;
pub mod vault_io;
#[cfg(test)]
mod workflow_facade_tests;

pub use model::*;
pub use mwt_base::{
    Amount, Error, Result, amount, calc, calendar, error, limits, routes, secret, time,
    utilities_formula,
};
pub use mwt_crypto as crypto;
pub use mwt_crypto::{
    SnapshotFormat, VAULT_CONTAINER_VERSION, VAULT_LEGACY_CONTAINER_VERSION, VAULT_MAGIC,
};
pub use mwt_household as household;
pub use mwt_model::{VAULT_FORMAT_VERSION, id_types, records};
pub use mwt_money as money;
pub use mwt_planning as planning;
pub use mwt_sync as sync;
pub use mwt_workflow as workflow;
pub use storage::VaultFile;

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

#[cfg(test)]
mod crypto_facade_tests {
    //! The paths of `mwt-crypto` that `ipc_contract` still names.

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
