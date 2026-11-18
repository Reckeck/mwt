//! Shim of the purchases of an event: moved to `crate::household::shared_events::purchases`
//! and `crate::usecases::household::shared_events` (spec core-strangler, D1).

use crate::error::Result;
use crate::ipc_contract::{AttachPaymentInput, SharedPurchaseDraft};

use super::super::UnlockedVault;
use super::super::forward::forward;
use super::SharedEventView;

impl UnlockedVault {
    // forward:begin
    forward! {
        pub fn add_shared_purchase(&mut self, event_id: &str, draft: SharedPurchaseDraft) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::add_shared_purchase;
        /// Change a purchase; its item stays. A payment the event made follows the
        /// change; an adopted payment keeps its own amount and only the number of
        /// units can change.
        pub fn update_shared_purchase(&mut self, event_id: &str, purchase_id: &str, draft: SharedPurchaseDraft) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::update_shared_purchase;
        /// Delete a purchase with the payment the event made for it; an adopted
        /// payment stays in the vault, unlinked.
        pub fn delete_shared_purchase(&mut self, event_id: &str, purchase_id: &str) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::delete_shared_purchase;
        /// Take a payment that is already in the vault as my purchase of the event.
        pub fn attach_payment_to_shared_event(&mut self, input: AttachPaymentInput) -> Result<SharedEventView>
            => crate::usecases::household::shared_events::attach_payment_to_shared_event;
    }
    // forward:end
}
