//! Shim of the `cryptotrack/positions` area: the rules and the reads moved to
//! `crate::cryptotrack::service::positions`, the scenario to
//! `crate::usecases::cryptotrack::positions` (spec core-strangler, D1).

pub use crate::cryptotrack::service::positions::CryptoPositionsApplied;

use crate::cryptotrack::protocols::{PositionReading, ProtocolReview};
use crate::error::Result;
use crate::ipc_contract::cryptotrack::positions::PositionView;
use crate::model::cryptotrack::CryptoTx;

// What the delegate below and the position tests reach through `super::*`.
#[cfg(test)]
use crate::Amount;
#[cfg(test)]
use crate::cryptotrack::engine::ids;
#[cfg(test)]
use crate::cryptotrack::protocols::{AssetAmount, PositionStatus, is_closed};
#[cfg(test)]
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoLeg, CryptoPosition, CryptoTxKind,
};

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Store what the protocol adapters read for one account: the positions
        /// and their operations. Saves once, and re-derives the holdings, because
        /// a position changes what the capital counts.
        ///
        /// `reviewed` is what each protocol that *answered* still sees
        /// ([`ProtocolReview`]). A stored position of this account that such a
        /// review no longer lists is closed: its legs go to zero and the row
        /// stays, so its history and the periods it was open in survive, and the
        /// next reading under the same id simply overwrites it. A protocol that
        /// refused has no review, and nothing is closed on its word.
        ///
        /// A reading of another account is ignored: a pass answers for exactly
        /// the account it was started for. A pass with nothing to change saves
        /// nothing.
        pub fn crypto_apply_positions(&mut self, account_id: &str, readings: &[PositionReading], events: &[CryptoTx], reviewed: &[ProtocolReview]) -> Result<CryptoPositionsApplied>
            => crate::usecases::cryptotrack::positions::crypto_apply_positions;
        /// Every stored position with its money, newest reading first.
        ///
        /// `live` is what this device's own last pass read
        /// (`crates/app/src/cryptotrack/jobs.rs`); an empty slice is a device
        /// that has only synced, and then the figures a reading alone can give
        /// are absent rather than zero.
        ///
        /// `at` is the moment an open position's holding period is measured to
        /// (RFC3339 UTC), the way [`UnlockedVault::crypto_totals`] takes one: the
        /// caller owns the clock, so the whole fold is a function of its inputs.
        /// It does not choose the prices — those are the newest the vault
        /// knows.
        pub fn crypto_list_positions(&self, live: &[PositionReading], at: &str) -> Vec<PositionView>
            => crate::usecases::cryptotrack::positions::crypto_list_positions @books;
    }
    // forward:end

    // Crate-private and test-only, so not in the baseline: outside the regenerated block.
    forward! {}
}

#[cfg(test)]
#[path = "positions_tests.rs"]
mod tests;
