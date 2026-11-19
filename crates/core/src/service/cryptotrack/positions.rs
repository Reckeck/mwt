//! Shim of the `cryptotrack/positions` area: the rules and the reads moved to
//! `crate::cryptotrack::service::positions`, the scenario to
//! `crate::usecases::cryptotrack::positions` (spec core-strangler, D1).

#[cfg(test)]
use std::collections::BTreeSet;

pub(crate) use crate::cryptotrack::service::positions::CryptoPositionInputs;
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

    // Called by the totals and the snapshots until those read the document
    // directly; not in the baseline (crate-private), so outside the regenerated block.
    forward! {
        /// Whether the capital counts what this account holds: it is followed
        /// and not archived. An address watched without being counted stays out.
        pub(crate) fn crypto_account_counts(&self, account_id: &str) -> bool
            => crate::cryptotrack::service::positions::crypto_account_counts @doc;
        /// What takes part in a position's value, built once for the present
        /// capital and the snapshots: positions of followed, live accounts, no
        /// stale twin of an old key, plus the receipts.
        pub(crate) fn crypto_position_inputs(&self) -> CryptoPositionInputs<'_>
            => crate::cryptotrack::service::positions::crypto_position_inputs @doc;
        /// Asset keys the module holds only as the receipt of a position (an
        /// aToken, stETH, a position NFT), so the capital counts the money once.
        #[cfg(test)]
        pub(crate) fn crypto_position_receipts(&self) -> BTreeSet<String>
            => crate::cryptotrack::service::positions::crypto_position_receipts @doc;
        /// USD for one unit now, by the same order of sources every other
        /// position of the app is valued at; `None`, never zero, when nothing
        /// knows it.
        #[cfg(test)]
        pub(crate) fn price_now_usd(&self, asset: &CryptoAsset) -> Option<Amount>
            => crate::cryptotrack::service::positions::price_now_usd @books;
    }
}

#[cfg(test)]
#[path = "positions_tests.rs"]
mod tests;
