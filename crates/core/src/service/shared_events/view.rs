//! Shim of the view of an event: moved to `crate::household::shared_events::view`
//! (spec core-strangler, D1).

pub use crate::household::shared_events::view::{
    ContactEventBalance, EventTotals, ItemShareView, ItemView, MyPosition, PersonView,
    PlannedPayment, PurchaseView, SettlementView, SharedEventSummary, SharedEventView,
    SharedTxLink,
};

#[cfg(test)]
use super::super::UnlockedVault;
#[cfg(test)]
use crate::error::Result;
#[cfg(test)]
use crate::model::shared_event::SharedEvent;

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
