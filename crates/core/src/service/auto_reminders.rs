//! Shim of the `auto_reminders` area: the rules moved to `crate::planning::auto_reminders`, the
//! scenarios to `crate::usecases::planning::auto_reminders` (spec core-strangler, D1).

pub use crate::planning::auto_reminders::category_of;
pub(crate) use crate::planning::auto_reminders::source_identity;

use crate::model::planning::Reminder;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The reminders as a reader in `ru` or English sees them. The stored
        /// text of an automatic reminder is in the language of whichever device
        /// wrote it; when it is one of the two renderings of its source it is
        /// shown in the reader's language, otherwise (the user typed it, or the
        /// data moved on) as it is. Nothing is written.
        pub fn reminders_in(&self, now: i64, zone: &str, ru: bool) -> Vec<Reminder>
            => crate::usecases::planning::auto_reminders::reminders_in;
        /// Create, move and remove the automatic reminders so they match the data.
        /// Returns whether anything changed (and was written).
        pub fn sync_auto_reminders(&mut self, now: i64, zone: &str, ru: bool) -> crate::Result<bool>
            => crate::usecases::planning::auto_reminders::sync_auto_reminders;
    }
    // forward:end
}
