//! Shim of the `cryptotrack/legacy_bybit` area: the rules moved to
//! `crate::cryptotrack::service::legacy_bybit`, the scenario to
//! `crate::usecases::cryptotrack::legacy_bybit` (spec core-strangler, D1).

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

    // Called by the exchange sync; not in the baseline
    // (crate-private), so outside the regenerated block.
    forward! {}
}

#[cfg(test)]
mod tests;
