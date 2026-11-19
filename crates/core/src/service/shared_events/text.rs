//! Shim of the share text of an event: moved to `crate::household::shared_events::text`
//! and `crate::usecases::household::shared_events` (spec core-strangler, D1).

use crate::error::Result;

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The settlement as text for a chat, in `ru` or `en`.
        pub fn shared_event_share_text(&self, event_id: &str, locale: &str) -> Result<String>
            => crate::usecases::household::shared_events::shared_event_share_text;
    }
    // forward:end
}
