//! Shim of the ledger side of an event: moved to `crate::household::shared_events::ledger`
//! and `crate::usecases::household::shared_events` (spec core-strangler, D1).

use crate::error::Result;
use crate::model::shared_event::SharedEvent;
use crate::model::transaction::Transaction;

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Drop the purchases and settlements of every event that point at an
        /// operation about to be deleted. Reopens an archived event. The caller saves.
        pub(crate) fn cascade_shared_delete(&mut self, tx_id: &str)
            => crate::usecases::household::shared_events::cascade_shared_delete;
        /// The event as it will be once the operation `tx_id` becomes `new_tx`;
        /// `None` when no event owns the operation.
        pub(crate) fn plan_shared_tx_edit(&self, tx_id: &str, new_tx: &Transaction) -> Result<Option<SharedEvent>>
            => crate::household::shared_events::ledger::plan_tx_edit @doc;
        /// Put the event a ledger edit planned in place of its older self.
        pub(crate) fn apply_shared_edit(&mut self, next: Option<SharedEvent>)
            => crate::usecases::household::shared_events::apply_shared_edit;
    }
    // forward:end
}
