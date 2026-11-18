//! Shim of the view of an event: moved to `crate::household::shared_events::view`
//! (spec core-strangler, D1).

pub use crate::household::shared_events::view::{
    ContactEventBalance, EventTotals, ItemShareView, ItemView, MyPosition, PersonKind, PersonView,
    PlannedPayment, PurchaseView, SettlementView, SharedEventSummary, SharedEventView,
    SharedTxLink,
};

use crate::error::Result;
use crate::household::shared_events::view::SharedDebt;
#[cfg(test)]
use crate::model::shared_event::SharedEvent;

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The event with everything worked out.
        pub fn shared_event_view(&self, id: &str) -> Result<SharedEventView>
            => crate::household::shared_events::view::shared_event_view @doc;
        /// The events, newest first by their date (`starts_on`, else the day they
        /// were made); archived ones only when asked for.
        pub fn list_shared_events(&self, include_archived: bool) -> Vec<SharedEventSummary>
            => crate::household::shared_events::view::list_shared_events @doc;
        /// The events that left a debt with (or credit from) a contact.
        pub fn contact_shared_events(&self, contact_id: &str) -> Vec<ContactEventBalance>
            => crate::household::shared_events::view::contact_shared_events @doc;
        /// Every event debt with a contact, for the contact balances: positive
        /// means the contact owes me.
        pub(crate) fn shared_debts(&self, secs: i64) -> Vec<SharedDebt>
            => crate::household::shared_events::view::shared_debts @doc;
        /// Every operation tied to an event, found through the events (the
        /// operations themselves carry no mark, so older vault files still read).
        pub fn shared_tx_links(&self) -> Vec<SharedTxLink>
            => crate::household::shared_events::view::shared_tx_links @doc;
    }
    // forward:end
}

/// The accessors the tests of the area use on the vault.
#[cfg(test)]
impl UnlockedVault {
    pub(crate) fn find_shared_event(&self, id: &str) -> Result<&SharedEvent> {
        crate::household::shared_events::find_event(&self.doc, id)
    }

    pub(crate) fn find_shared_event_mut(&mut self, id: &str) -> Result<&mut SharedEvent> {
        self.doc
            .shared_events
            .iter_mut()
            .find(|event| event.id == id)
            .ok_or_else(|| crate::error::Error::Missing(format!("shared event {id}")))
    }
}
