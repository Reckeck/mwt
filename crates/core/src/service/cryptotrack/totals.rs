//! Shim of the `cryptotrack/totals` area: the rules and queries moved to
//! `crate::cryptotrack::service::totals`, the scenarios to
//! `crate::usecases::cryptotrack::recompute` (spec core-strangler, D1).

// The positions tests name it through the shim.
#[cfg(test)]
pub use crate::cryptotrack::service::totals::CryptoLeftOut;
use crate::cryptotrack::service::totals::CryptoOutsideTotals;

use crate::cryptotrack::service::totals::CryptoHistory;
#[cfg(test)]
use crate::model::cryptotrack::CryptoAsset;
#[cfg(test)]
use crate::model::store::AssetRef;

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

    // Called by the holdings fold, the reports and the positions until those areas move;
    // not in the baseline (crate-private), so outside the regenerated block.
    forward! {
        /// Put the engine's balances into `doc.holdings`, replacing what they
        /// supersede. Called at the end of `recompute_holdings`; saves nothing.
        pub(crate) fn apply_crypto_holdings(&mut self)
            => crate::usecases::cryptotrack::recompute::apply_crypto_holdings;
        /// What the module was worth at each cutoff of a report, for the months a
        /// replay of the transactions cannot reach. `in_scope` is the report's own
        /// wallet filter.
        pub(crate) fn crypto_history(&self, cutoffs: &[i64], in_scope: impl Fn(&str) -> bool) -> CryptoHistory
            => crate::cryptotrack::service::totals::crypto_history @books;
        /// Tokens the module met and refuses to fold, named so a total can say
        /// what it left out.
        pub(crate) fn crypto_quarantined(&self) -> Vec<CryptoOutsideTotals>
            => crate::cryptotrack::service::totals::crypto_quarantined @doc;
        /// The identity a module holding is shown and priced under.
        #[cfg(test)]
        pub(crate) fn crypto_asset_ref(&self, asset: &CryptoAsset) -> AssetRef
            => crate::cryptotrack::service::totals::crypto_asset_ref @doc;
    }
}

#[cfg(test)]
mod tests;
