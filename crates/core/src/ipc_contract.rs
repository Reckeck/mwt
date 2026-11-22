//! The IPC contract: the crate `mwt-contract` plus the types that name `appconfig`, which owns file IO and
//! stays in the facade (it imports `AutolockSettings` from the contract, so the contract cannot name it).
//! Every old path `ipc_contract::<item>` still resolves.

use serde::{Deserialize, Serialize};

pub use crate::appconfig::{
    AppSettings, Locale, SupportedCurrencies, Theme, UpdateAppSettingsInput,
};
pub use mwt_contract::*;

/// Catalog of currencies the base/favourite pickers may offer.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct CurrencyCatalogOutput {
    pub catalog: SupportedCurrencies,
    /// Machine code describing where the list came from: `builtin` when it is
    /// the built-in fallback (never fetched), `cached` when the network was
    /// unreachable and the last successful fetch was reused, `null` when the
    /// providers answered just now.
    pub notice: Option<String>,
}
