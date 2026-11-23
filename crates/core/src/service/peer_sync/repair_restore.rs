//! Shim of the `peer_sync/repair_restore` area: moved to `crate::sync::repair::repair_restore` (spec core-strangler, D1).

use super::UnlockedVault;
use crate::service::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Whether a live record of the event still names the participant.
        #[cfg(test)]
        pub(crate) fn event_names_participant(&self, event_id: &str, participant_id: &str) -> bool
            => crate::sync::repair::repair_restore::event_names_participant @doc;
        /// Whether money still names the record under this sync key
        /// (`stores:<id>`, `categories:<key>`, `contacts:<id>`, and
        /// `transactions:<id>` for a payment a shared event names).
        #[cfg(test)]
        pub(crate) fn money_names_record(&self, key: &str) -> bool
            => crate::sync::repair::repair_restore::money_names_record @doc;
    }
    // forward:end
}
