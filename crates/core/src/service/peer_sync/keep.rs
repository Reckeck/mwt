//! Shim of the `peer_sync/keep` area: moved to `crate::sync::repair::keep` (spec core-strangler, D1).

use crate::model::planning::Reminder;
use crate::planning::auto_reminders::source_identity;

/// An automatic reminder is one per source (kind + entity id), however its id
/// or its key was made. Reminders of the old build (random id, schedule-only
/// key) group with the new ones by their source.
pub(super) fn reminder_identity(reminder: &Reminder) -> Option<String> {
    match &reminder.source {
        Some(source) => Some(source_identity(source)),
        None => (!reminder.source_key.is_empty()).then(|| reminder.source_key.clone()),
    }
}
