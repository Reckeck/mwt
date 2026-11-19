//! Shim of the `adjustments` area: moved to `crate::money::adjustments` (spec
//! core-strangler, D1). The corrections are worked out there; the scenario that
//! rebases them over the vault is in `usecases::money::adjustments`.

pub use crate::money::adjustments::AdjustmentDraft;

use crate::Amount;
use crate::error::Result;
use crate::ipc_contract::AdjustmentDraftInput;
use crate::model::store::AssetRef;
use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Balance of every asset of one store at `at` (inclusive), leaving out the
        /// transaction `exclude`. Zero balances are dropped.
        pub fn balances_at(&self, store_id: &str, at: &str, exclude: Option<&str>) -> Vec<(AssetRef, Amount)>
            => crate::money::adjustments::balances_at @doc;
        /// The correction screen of one account: its assets and their ledger
        /// balances at the moment asked (now by default).
        pub fn adjustment_draft(&self, input: &AdjustmentDraftInput) -> Result<AdjustmentDraft>
            => crate::usecases::money::adjustments::adjustment_draft @doc;
        /// Work out again what the ledger held before every correction.
        pub(crate) fn rebase_adjustments(&mut self)
            => crate::usecases::money::adjustments::rebase_adjustments;
        /// Fill in what the ledger held before the correction, from the other
        /// transactions, and drop the assets whose balance already matches.
        pub(crate) fn complete_adjustment(&self, tx: &mut Transaction) -> Result<()>
            => crate::money::adjustments::complete_adjustment @doc;
    }
    // forward:end
}
