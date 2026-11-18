//! Shim of the shared events area (event, people, items, split, settling): moved to
//! `crate::household::shared_events` and `crate::usecases::household::shared_events`
//! (spec core-strangler, D1). The purchases, ledger, view, text and analytics
//! parts have not moved yet.

use crate::error::Result;
use crate::ipc_contract::{
    SetShareOverrideInput, SetSharedItemSplitInput, SharedEventDraft, SharedItemDraft,
    SharedParticipantDraft,
};
use crate::model::contact::Contact;

use super::UnlockedVault;
use super::forward::forward;

mod analytics;
pub(crate) use analytics::SharedExtra;
mod ledger;
mod purchases;
#[cfg(test)]
pub(crate) use crate::household::shared_events::testing;
mod text;
mod view;

mod settle;
pub(crate) mod split;
pub(crate) use crate::household::shared_events::{FALLBACK_CATEGORY, item_category};
pub use view::{
    ContactEventBalance, EventTotals, ItemShareView, ItemView, MyPosition, PersonKind, PersonView,
    PlannedPayment, PurchaseView, SettlementView, SharedEventSummary, SharedEventView,
    SharedTxLink,
};

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Save after a change to the event: an archived event that was edited is
        /// open again.
        pub(crate) fn persist_event(&mut self, id: &str) -> Result<()>
            => crate::usecases::household::shared_events::persist_event;
        pub fn create_shared_event(&mut self, draft: SharedEventDraft) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::create_shared_event;
        /// Change the event's own fields. Things the purchases and settlements
        /// already depend on stay put: the currency, who organizes, whether I take
        /// part (while it would strand my payments).
        pub fn update_shared_event(&mut self, id: &str, draft: SharedEventDraft) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::update_shared_event;
        /// Delete an event. Every real operation it made or adopted stays in the
        /// vault as an ordinary one: my payments are plain payments, and the
        /// `Collect`/`Repay` transfers are plain debt transfers (they count in the
        /// contact balances again, as the event's own balance goes away).
        pub fn delete_shared_event(&mut self, id: &str) -> Result<()>
            => crate::usecases::household::shared_events::delete_shared_event;
        /// Put a settled event away (or take it back).
        pub fn archive_shared_event(&mut self, id: &str, archived: bool) -> Result<()>
            => crate::usecases::household::shared_events::archive_shared_event;
        pub fn add_shared_participant(&mut self, event_id: &str, draft: SharedParticipantDraft) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::add_shared_participant;
        /// Rename a guest, change their details, or link them to a contact
        /// (their debts then show in the contact's card).
        pub fn update_shared_participant(&mut self, event_id: &str, participant_id: &str, draft: SharedParticipantDraft) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::update_shared_participant;
        /// Take someone out of the event; refused once money or a hand-set total
        /// mentions them.
        pub fn remove_shared_participant(&mut self, event_id: &str, participant_id: &str) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::remove_shared_participant;
        /// "Make a contact": a guest becomes a contact of the directory with
        /// the details they had, and is linked to it.
        pub fn make_shared_participant_contact(&mut self, event_id: &str, participant_id: &str) -> Result<Contact>
            => crate::usecases::household::shared_events::make_shared_participant_contact;
        /// Fix a person's total by hand (the rest is spread over the others), or
        /// take the fixed total back with `None`.
        pub fn set_shared_share_override(&mut self, input: SetShareOverrideInput) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::set_shared_share_override;
        pub fn add_shared_item(&mut self, event_id: &str, draft: SharedItemDraft) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::add_shared_item;
        pub fn update_shared_item(&mut self, event_id: &str, item_id: &str, draft: SharedItemDraft) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::update_shared_item;
        /// Delete an item nothing was bought for.
        pub fn delete_shared_item(&mut self, event_id: &str, item_id: &str) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::delete_shared_item;
        /// Who shares an item and how.
        pub fn set_shared_item_split(&mut self, input: SetSharedItemSplitInput) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::set_shared_item_split;
    }
    // forward:end
}
