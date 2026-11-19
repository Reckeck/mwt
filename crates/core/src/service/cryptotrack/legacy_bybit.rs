//! Shim of the `cryptotrack/legacy_bybit` area: the rules moved to
//! `crate::cryptotrack::service::legacy_bybit`, the scenario to
//! `crate::usecases::cryptotrack::legacy_bybit` (spec core-strangler, D1).

use std::collections::BTreeSet;

use crate::cryptotrack::service::legacy_bybit::BybitStored;
use crate::error::Result;
use crate::model::vault::PendingImport;

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Whether a wallet is read by the crypto module rather than kept by
        /// hand: an exchange with a key or with a followed account of its own.
        ///
        /// Spec E11 archives the old hand-kept crypto wallets when the user moves
        /// to level 3; a wallet the engine drives must stay, because its
        /// operations are refetched and its balance is recomputed. This is the
        /// mark E11 reads.
        pub fn exchange_is_engine_driven(&self, store_id: &str) -> bool
            => crate::usecases::cryptotrack::legacy_bybit::exchange_is_engine_driven @doc;
    }
    // forward:end

    // Called by the exchange sync until that area moves; not in the baseline
    // (crate-private), so outside the regenerated block.
    forward! {
        /// `exec_id`s of the exchange operations the module already holds.
        pub(crate) fn bybit_event_ids(&self) -> BTreeSet<String>
            => crate::cryptotrack::service::legacy_bybit::bybit_event_ids @doc;
        /// Store exchange records as operations of the module. The one way in:
        /// both a fresh sweep and the conversion of the old queue come through
        /// here, so neither can write an operation the other would not.
        ///
        /// Does not save; the caller does, once.
        pub(crate) fn bybit_store(&mut self, store_id: &str, records: &[PendingImport]) -> Result<BybitStored>
            => crate::usecases::cryptotrack::legacy_bybit::bybit_store;
    }
}

#[cfg(test)]
mod tests;
