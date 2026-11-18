//! Shim of the `validate` area: moved to `crate::money::validate` (spec core-strangler, D1).

use crate::error::Result;
use crate::model::store::Store;
use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Fill empty categories from the first matching active rule (plan §8.1):
        /// a case-insensitive substring of the comment or of a contact's name.
        pub(crate) fn apply_categorization_rules(&self, tx: &mut Transaction)
            => crate::money::validate::apply_categorization_rules @doc;
        pub(crate) fn live_store(&self, id: &str) -> Result<&Store>
            => crate::money::validate::live_store @doc;
        /// Plan §4.5, V1–V14 (V13 — negative balances — is a warning, not an error).
        pub(crate) fn validate_transaction(&self, tx: &Transaction) -> Result<()>
            => crate::usecases::household::validate::validate_transaction;
        /// The same rules, with V11 measured from `now_ms` instead of the wall
        /// clock: an automation run is checked against the scheduler's clock,
        /// which runs ahead of it in a simulation.
        pub(crate) fn validate_transaction_at(&self, tx: &Transaction, now_ms: i64) -> Result<()>
            => crate::usecases::household::validate::validate_transaction_at;
    }
    // forward:end

    /// Trim, lowercase and deduplicate category keys, keeping their order.
    pub(crate) fn normalize_categories(tx: &mut Transaction) {
        crate::money::validate::normalize_categories(tx);
    }

    /// Every store a transaction is booked on.
    pub(crate) fn store_sides(tx: &Transaction) -> Vec<&str> {
        crate::money::validate::store_sides(tx)
    }
}
