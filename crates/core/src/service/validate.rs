//! Shim of the `validate` area: moved to `crate::money::validate` (spec core-strangler, D1).

use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {}
    // forward:end

    /// Trim, lowercase and deduplicate category keys, keeping their order.
    pub(crate) fn normalize_categories(tx: &mut Transaction) {
        crate::money::validate::normalize_categories(tx);
    }

    /// Every store a transaction is booked on.
    #[cfg(test)]
    pub(crate) fn store_sides(tx: &Transaction) -> Vec<&str> {
        crate::money::validate::store_sides(tx)
    }
}
