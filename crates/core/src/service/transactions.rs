//! Shim of the `transactions` area: moved to `crate::money::transactions` (spec
//! core-strangler, D1). The scenarios that add, edit and delete operations live
//! in `usecases::money::transactions`.

use crate::error::Result;
use crate::model::store::StoreScope;
use crate::model::transaction::Transaction;
use crate::money::transactions::TransactionPatch;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Transaction history, newest first, limited to a scope.
        pub fn transactions(&self, scope: &StoreScope) -> Vec<Transaction>
            => crate::usecases::money::transactions::transactions;
        /// Everything a write does before storing: categories normalized and
        /// auto-filled, property prices completed, unpriced sides stamped, every
        /// rule checked.
        pub(crate) fn prepare_transaction(&self, tx: &mut Transaction) -> Result<()>
            => crate::usecases::money::transactions::prepare_transaction;
        /// [`Self::prepare_transaction`] with the future-date rule (V11) measured
        /// from `now_ms` — the scheduler's clock for automation runs.
        pub(crate) fn prepare_transaction_at(&self, tx: &mut Transaction, now_ms: i64) -> Result<()>
            => crate::usecases::money::transactions::prepare_transaction_at;
        /// Validate and append a transaction of any variant (plan §4.5, §8.1):
        /// categories are normalized and auto-filled, then every rule is checked.
        pub fn add_transaction(&mut self, tx: Transaction) -> Result<Transaction>
            => crate::usecases::money::transactions::add_transaction;
        /// [`Self::add_transaction`], linked to a property (plan G4).
        pub fn add_transaction_for(&mut self, tx: Transaction, property_id: Option<&str>) -> Result<Transaction>
            => crate::usecases::money::transactions::add_transaction_for;
        /// Save several operations as one movement of money (an exchange split
        /// between recipients, a P2P deal): all join one chain — `flow_id`, or a
        /// new one — and either all are saved or none is.
        pub fn add_linked_transactions(&mut self, txs: Vec<Transaction>, flow_id: Option<&str>) -> Result<Vec<Transaction>>
            => crate::usecases::money::transactions::add_linked_transactions;
        /// Replace a transaction entirely. Its id and origin are kept: an edit
        /// changes what happened, not where the record came from.
        pub fn update_transaction(&mut self, id: &str, tx: Transaction) -> Result<Transaction>
            => crate::usecases::money::transactions::update_transaction;
        /// Change an operation by naming only what changes (F4): every link the patch
        /// does not name keeps its stored value, the property link included.
        pub fn patch_transaction(&mut self, id: &str, patch: TransactionPatch) -> Result<Transaction>
            => crate::usecases::money::transactions::patch_transaction;
        /// [`Self::update_transaction`] for the shared event that owns the
        /// operation: the event has changed its own side already.
        pub(crate) fn update_transaction_unsynced(&mut self, id: &str, tx: Transaction) -> Result<Transaction>
            => crate::usecases::money::transactions::update_transaction_unsynced;
        /// [`Self::update_transaction`] that also sets the operation's property;
        /// `None` unlinks it.
        pub fn update_transaction_for(&mut self, id: &str, tx: Transaction, property_id: Option<&str>) -> Result<Transaction>
            => crate::usecases::money::transactions::update_transaction_for;
        pub fn delete_transaction(&mut self, id: &str) -> Result<()>
            => crate::usecases::money::transactions::delete_transaction;
    }
    // forward:end
}
