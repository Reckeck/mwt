//! Shim of the `peer_sync/keep` area: moved to `crate::sync::repair::keep` (spec core-strangler, D1).

use super::UnlockedVault;
use crate::error::Result;
use crate::ipc_contract::MergeTarget;
use crate::model::planning::Reminder;
use crate::service::auto_reminders::source_identity;
use crate::service::forward::forward;

/// An automatic reminder is one per source (kind + entity id), however its id
/// or its key was made. Reminders of the old build (random id, schedule-only
/// key) group with the new ones by their source.
pub(super) fn reminder_identity(reminder: &Reminder) -> Option<String> {
    match &reminder.source {
        Some(source) => Some(source_identity(source)),
        None => (!reminder.source_key.is_empty()).then(|| reminder.source_key.clone()),
    }
}

impl UnlockedVault {
    // forward:begin
    forward! {
        /// «Allow merging»: clear the mark, so the next sync may fold the record
        /// into its twin again. A local edit like any other.
        pub fn allow_merging(&mut self, target: MergeTarget, id: &str) -> Result<()>
            => crate::usecases::sync::repair::allow_merging;
    }
    // forward:end
}
