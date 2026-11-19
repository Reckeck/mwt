//! Shim of the `cryptotrack/recompute` area: the rules and queries moved to
//! `crate::cryptotrack::service::recompute`, the scenarios to
//! `crate::usecases::cryptotrack::recompute` (spec core-strangler, D1).

#[cfg(test)]
use crate::cryptotrack::engine::Ledger;
use crate::cryptotrack::service::recompute::{CryptoApplied, CryptoBatch, CryptoTotals};
use crate::cryptotrack::sources::evm::SourceBatch;
use crate::error::Result;
use crate::model::cryptotrack::CryptoSnapshot;

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Holdings, cost and profit as of `at`.
        pub fn crypto_totals(&self, at: &str) -> CryptoTotals
            => crate::cryptotrack::service::recompute::crypto_totals @books;
        /// Store a source's batch, fold everything again, reconcile with the
        /// network and snapshot the value. Saves once, and only if the batch
        /// changed something (a pass that met nothing new writes nothing).
        pub fn crypto_apply_batch(&mut self, request: &CryptoBatch<'_>, batch: &SourceBatch) -> Result<CryptoApplied>
            => crate::usecases::cryptotrack::recompute::crypto_apply_batch;
        /// Write the snapshots of `at` and save, if any of them is new.
        pub fn crypto_snapshot(&mut self, at: &str) -> Result<Vec<CryptoSnapshot>>
            => crate::usecases::cryptotrack::recompute::crypto_snapshot;
        /// Fold the engine again into `doc.holdings` and save: what the module's
        /// own writers call after they changed an event, a verdict or an account,
        /// since nothing else re-derives the holdings for them.
        pub fn crypto_sync_holdings(&mut self) -> Result<()>
            => crate::usecases::cryptotrack::recompute::crypto_sync_holdings;
    }
    // forward:end

    // Called by the token tests until they leave the vault; not in the baseline
    // (crate-private), so outside the regenerated block.
    forward! {
        /// Fold every operation of the module, with the corrections on top.
        #[cfg(test)]
        pub(crate) fn crypto_fold(&self) -> Ledger
            => crate::cryptotrack::service::recompute::fold @books;
    }
}

#[cfg(test)]
mod tests;
