//! Shim of the `cryptotrack/transition` area: the rules moved to
//! `crate::cryptotrack::service::transition`, the scenarios to
//! `crate::usecases::cryptotrack::transition` (spec core-strangler, D1).

use crate::error::Result;
use crate::ipc_contract::cryptotrack::transition::{CryptoArchiveReport, CryptoTransitionPlan};

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// What entering the module would change: the hand-kept crypto wallets the
        /// move archives, and how much capital leaves the totals with them.
        ///
        /// Computed every time and stored nowhere, so the wizard can ask for it at
        /// any level. A wallet the module reads itself is not in the list — an
        /// exchange with a working key stays and becomes a source of the engine,
        /// and nothing of it leaves.
        pub fn crypto_transition_plan(&self) -> CryptoTransitionPlan
            => crate::usecases::cryptotrack::transition::crypto_transition_plan @books;
        /// Archive the crypto wallets kept by hand and save. What the wizard's
        /// last step calls; idempotent, so calling it again writes nothing.
        pub fn crypto_archive_manual_wallets(&mut self) -> Result<CryptoArchiveReport>
            => crate::usecases::cryptotrack::transition::crypto_archive_manual_wallets;
    }
    // forward:end

    // Called by the raise to level 3 and the open chain; not in the baseline
    // (crate-private), so outside the regenerated block.
    forward! {}
}

#[cfg(test)]
mod tests;
