//! Shim of the `exchange` area (sync): the rules moved to
//! `crate::cryptotrack::exchange_sync`, the scenarios to
//! `crate::usecases::cryptotrack::exchange_sync` (spec core-strangler, D1).

use crate::error::Result;
use crate::exchange::CoinBalance;
use crate::model::store::ImportSettings;
use crate::model::vault::{ImportRun, ImportSource, PendingImport};

pub use crate::cryptotrack::exchange_sync::{IMPORT_ACCOUNTS, ImportPreview, ImportedTransactions};

// What the sync tests reach through `super::*`.
#[cfg(test)]
use crate::model::category::EXCHANGE_IMPORT_CATEGORY;
#[cfg(test)]
use crate::model::store::StoreKind;
#[cfg(test)]
use crate::model::transaction::{Transaction, TxOrigin};

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The import settings of the exchange wallet `store_id` belongs to.
        pub fn import_settings(&self, store_id: &str) -> Result<ImportSettings>
            => crate::cryptotrack::exchange_sync::import_settings @books;
        /// Change what an import brings in and where it books it.
        pub fn set_import_settings(&mut self, store_id: &str, mut settings: ImportSettings) -> Result<ImportSettings>
            => crate::usecases::cryptotrack::exchange_sync::set_import_settings;
        /// One sync of an exchange wallet: leave out what the settings do not
        /// want and what was seen before, record the rest as operations of the
        /// crypto module, and keep the sync in the history.
        ///
        /// Nothing is staged any more (spec E7): a record becomes an operation at
        /// once, so there is no queue to confirm and no verification screen.
        pub fn sync_exchange(&mut self, store_id: &str, imports: Vec<PendingImport>, source: ImportSource, now: i64) -> Result<ImportRun>
            => crate::usecases::cryptotrack::exchange_sync::sync_exchange;
        /// What a sync would bring under `settings` (the saved ones, or the ones
        /// still being edited): nothing is staged or kept.
        pub fn preview_exchange_sync(&self, imports: Vec<PendingImport>, settings: &ImportSettings) -> ImportPreview
            => crate::cryptotrack::exchange_sync::preview_exchange_sync @books;
        /// A sync that did not go through, for the history.
        pub fn record_import_error(&mut self, store_id: &str, source: ImportSource, error: &str, now: i64) -> Result<ImportRun>
            => crate::usecases::cryptotrack::exchange_sync::record_import_error;
        /// Syncs of one exchange wallet, newest first.
        pub fn import_runs(&self, store_id: &str) -> Vec<ImportRun>
            => crate::cryptotrack::exchange_sync::import_runs @books;
        /// Operations brought in from the exchange wallet `store_id` belongs to
        /// (its own and its sub-accounts'), newest first.
        pub fn imported_transactions(&self, store_id: &str, limit: usize, offset: usize) -> ImportedTransactions
            => crate::cryptotrack::exchange_sync::imported_transactions @books;
        /// Set the live balance of each exchange account where the ledger
        /// disagrees with it, by one correction per sub-account.
        pub fn reconcile_exchange(&mut self, store_id: &str, balances: &[(String, Vec<CoinBalance>)], now: i64) -> Result<Vec<String>>
            => crate::usecases::cryptotrack::exchange_sync::reconcile_exchange;
    }
    // forward:end

    // Not in the baseline (crate-private), so outside the regenerated block.
    forward! {
        /// Ids an import has already settled: an operation of the crypto module,
        /// a ledger transaction the old import booked, or a record the user refused.
        pub(crate) fn decided_import_ids(&self) -> std::collections::HashSet<String>
            => crate::cryptotrack::exchange_sync::decided_import_ids @doc;
    }

    /// Discarded ids are remembered, so the next sync does not stage them again.
    #[cfg(test)]
    pub(crate) fn remember_discarded(&mut self, ids: impl IntoIterator<Item = String>) {
        crate::cryptotrack::exchange_sync::remember_discarded(&mut self.doc, ids);
    }
}

#[cfg(test)]
#[path = "exchange_sync_tests.rs"]
mod tests;
