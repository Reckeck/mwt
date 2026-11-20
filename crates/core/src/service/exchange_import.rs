//! Shim of the `exchange` area (import and terminal): the rules moved to
//! `crate::cryptotrack::exchange_import`, the scenarios to
//! `crate::usecases::cryptotrack::exchange_import` (spec core-strangler, D1).

use crate::error::Result;
use crate::model::store::TerminalState;
use crate::model::vault::PendingImport;

use super::forward::forward;
use super::{TerminalPatch, UnlockedVault};

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Status of the Bybit key stored for exactly this store.
        pub fn exchange_key_status(&self, store_id: &str) -> crate::ipc_contract::ExchangeKeyStatus
            => crate::usecases::cryptotrack::exchange_import::exchange_key_status;
        /// Store (or replace) a Bybit key for an exchange store in the device
        /// secrets file, unverified. Level 3 only.
        pub fn set_exchange_key(&mut self, store_id: &str, api_key: &str, api_secret: &str) -> Result<crate::ipc_contract::ExchangeKeyStatus>
            => crate::usecases::cryptotrack::exchange_import::set_exchange_key;
        /// Point imported spot / futures fills at sub-accounts of the key's
        /// store. Each target must be an exchange store inside that wallet.
        pub fn set_exchange_targets(&mut self, store_id: &str, spot_store_id: Option<String>, futures_store_id: Option<String>) -> Result<crate::ipc_contract::ExchangeKeyStatus>
            => crate::usecases::cryptotrack::exchange_import::set_exchange_targets;
        /// Stage fills for user confirmation. Returns how many fills were newly staged.
        pub fn stage_imports(&mut self, imports: Vec<PendingImport>) -> Result<u32>
            => crate::usecases::cryptotrack::exchange_import::stage_imports;
        /// Every staged fill, newest first.
        pub fn pending_imports(&self) -> Vec<PendingImport>
            => crate::cryptotrack::exchange_import::pending_imports @doc;
        /// The Bybit key that serves a store: its own, else its wallet's.
        pub fn bybit_key_for(&self, store_id: &str) -> Option<&crate::model::vault::ExchangeKeys>
            => crate::cryptotrack::exchange_import::bybit_key_for @books;
        /// Give a Bybit wallet its three accounts, creating the ones missing.
        pub fn ensure_bybit_accounts(&mut self, store_id: &str, names: [&str; 3]) -> Result<[String; 3]>
            => crate::usecases::cryptotrack::exchange_import::ensure_bybit_accounts;
        /// [`Self::ensure_bybit_accounts`] for the accounts `wanted`.
        pub fn ensure_bybit_accounts_for(&mut self, store_id: &str, names: [&str; 3], wanted: [bool; 3]) -> Result<[Option<String>; 3]>
            => crate::usecases::cryptotrack::exchange_import::ensure_bybit_accounts_for;
        /// The live Unified Trading account of an exchange wallet.
        pub fn unified_account_of(&self, store_id: &str) -> Option<String>
            => crate::cryptotrack::exchange_import::unified_account_of @books;
        /// What each Bybit account holds per the API next to what the ledger
        /// says its sub-account holds, coin by coin.
        pub fn bybit_reconcile(&self, store_id: &str, balances: &[(String, Vec<crate::exchange::CoinBalance>)]) -> Vec<crate::ipc_contract::ExchangeAccountBalance>
            => crate::usecases::cryptotrack::exchange_import::bybit_reconcile @books;
        /// Background import interval of one exchange; `0` disables it.
        pub fn set_import_interval(&mut self, store_id: &str, interval_sec: u32) -> Result<()>
            => crate::usecases::cryptotrack::exchange_import::set_import_interval;
        /// Exchanges with a key on this device whose background import is on.
        pub fn import_schedule(&self) -> Vec<(u32, crate::vault_io::device_secrets::DeviceKey)>
            => crate::usecases::cryptotrack::exchange_import::import_schedule;
        pub fn terminal_state(&self, store_id: &str) -> Result<TerminalState>
            => crate::cryptotrack::exchange_import::terminal_state @doc;
        /// Patch the terminal of one exchange; only the given fields change.
        pub fn update_terminal_state(&mut self, store_id: &str, patch: TerminalPatch) -> Result<TerminalState>
            => crate::usecases::cryptotrack::exchange_import::update_terminal_state;
        /// Replace one symbol's drawings; an empty export removes the entry.
        pub fn save_terminal_drawings(&mut self, store_id: &str, symbol: &str, json: &str) -> Result<()>
            => crate::usecases::cryptotrack::exchange_import::save_terminal_drawings;
        /// Turn staged fills into real trades.
        pub fn confirm_pending_imports(&mut self, exec_ids: &[String]) -> Result<u32>
            => crate::usecases::cryptotrack::exchange_import::confirm_pending_imports;
        /// Drop staged fills without creating transactions.
        pub fn discard_pending_imports(&mut self, exec_ids: &[String]) -> Result<u32>
            => crate::usecases::cryptotrack::exchange_import::discard_pending_imports;
    }
    // forward:end
}
