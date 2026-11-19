//! Shim of the `cryptotrack/apply` area: the rules moved to `crate::cryptotrack::service::apply`,
//! the scenarios to `crate::usecases::cryptotrack::apply` (spec core-strangler, D1).

use crate::cryptotrack::sources::evm::SourceBatch;
use crate::error::Result;
use crate::ipc_contract::cryptotrack::sync::{CryptoPages, CryptoPagesApplied, CryptoSyncTarget};
use crate::model::cryptotrack::CryptoPricePoint;

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The accounts a pass of the tracking reads. An archived account is
        /// watched but not followed, and an exchange account is another spec's
        /// source (E7); only the EVM networks have a source here.
        pub fn crypto_sync_targets(&self) -> Vec<CryptoSyncTarget>
            => crate::usecases::cryptotrack::apply::crypto_sync_targets @doc;
        /// Keep what the protocol adapters want to remember about an account
        /// (`None` forgets), beside the source cursors and as local as they are:
        /// a block a scan has reached names this device's own reading. Saves
        /// only when something differs.
        pub fn crypto_remember_protocols(&mut self, account_id: &str, memory: &[(String, Option<String>)]) -> Result<()>
            => crate::usecases::cryptotrack::apply::crypto_remember_protocols;
        /// Store pages a source read and move the account's cursor to where the
        /// batch ends, in the same save: an interruption resumes exactly after
        /// what was stored, and a page read twice produces the same operations.
        ///
        /// A failed store puts the cursor back, so the next pass reads the same
        /// pages again.
        pub fn crypto_apply_pages(&mut self, request: &CryptoPages<'_>, batch: &SourceBatch) -> Result<CryptoPagesApplied>
            => crate::usecases::cryptotrack::apply::crypto_apply_pages;
        /// Forget where an account's read stands, so the next pass starts over
        /// (the user asked to read it again).
        pub fn crypto_reset_cursor(&mut self, account_id: &str) -> Result<()>
            => crate::usecases::cryptotrack::apply::crypto_reset_cursor;
        /// Store the historic prices a pass fetched (spec D7, wave 0), saving
        /// once and only if something changed. Returns how many points are new or
        /// different.
        pub fn crypto_apply_prices(&mut self, fresh: &[CryptoPricePoint]) -> Result<usize>
            => crate::usecases::cryptotrack::apply::crypto_apply_prices;
    }
    // forward:end
}
