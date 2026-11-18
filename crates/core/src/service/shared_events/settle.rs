//! Shim of settling an event: moved to `crate::household::shared_events::settle` and
//! `crate::usecases::household::shared_events` (spec core-strangler, D1).

use crate::error::Result;
use crate::ipc_contract::{SetSharedDueInput, SettleSharedInput, SharedRemainderInput};
use crate::model::contact::DebtTerms;

use super::super::UnlockedVault;
use super::super::forward::forward;
use super::view::SharedEventView;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Money between a participant and the organizer (or a set-off).
        pub fn settle_shared(&mut self, input: SettleSharedInput) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::settle_shared;
        /// "Write off the rest": I give up what a participant owes me; it becomes my
        /// expense in the category of the event.
        pub fn write_off_shared(&mut self, input: SharedRemainderInput) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::write_off_shared;
        /// "Move onto the others": what a participant has not returned is split
        /// between the others.
        pub fn reassign_shared(&mut self, input: SharedRemainderInput) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::reassign_shared;
        /// Take a settlement back, with the operation it made.
        pub fn delete_shared_settlement(&mut self, event_id: &str, settlement_id: &str) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::delete_shared_settlement;
        /// The due date of a contact participant's debt (the existing debt terms,
        /// keyed by contact and currency); an empty date clears it.
        pub fn set_shared_due(&mut self, input: SetSharedDueInput) -> Result<Option<DebtTerms>>
            => crate::usecases::household::shared_events::set_shared_due;
    }
    // forward:end
}
