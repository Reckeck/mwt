//! Shim of the `cryptotrack/totals` area: the rules and queries moved to
//! `crate::cryptotrack::service::totals`, the scenarios to
//! `crate::usecases::cryptotrack::recompute` (spec core-strangler, D1).

// The positions tests name it through the shim.
#[cfg(test)]
pub use crate::cryptotrack::service::totals::CryptoLeftOut;
use crate::cryptotrack::service::totals::CryptoOutsideTotals;

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// What the module knows and the capital does not count, with the reason.
        pub fn crypto_outside_totals(&self) -> Vec<CryptoOutsideTotals>
            => crate::cryptotrack::service::totals::crypto_outside_totals @books;
    }
    // forward:end

    // Called by the holdings fold and the positions until those areas move;
    // not in the baseline (crate-private), so outside the regenerated block.
    forward! {}
}

#[cfg(test)]
mod tests;
