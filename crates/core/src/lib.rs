//! Multi Wallet Tracker `core`: the facade. Every module path that `app`, `xtask` and the tests use
//! (`trade_core::service`, `trade_core::usecases`, `trade_core::model`, ...) resolves to `mwt-usecases`,
//! which holds the code. This crate MUST NOT depend on Tauri or any UI runtime.

pub use mwt_usecases::*;
