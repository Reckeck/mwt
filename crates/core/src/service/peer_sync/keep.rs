//! The «keep» mark (stage M): a copy the user got back by undoing a sync that
//! had merged it away. The dedupe skips every pair that holds a marked record,
//! so the next batch does not remove the copy a second time. The mark is an
//! ordinary synced field; a plain edit leaves it, «Allow merging» clears it.

use std::collections::HashSet;

use super::UnlockedVault;
use crate::error::{Error, Result};
use crate::ipc_contract::MergeTarget;
use crate::model::planning::Reminder;
use crate::model::transaction::TxOrigin;
use crate::service::auto_reminders::source_identity;

/// What two operations must share for the dedupe to treat them as one made
/// twice (an import, an automation moment or a workflow run), and the run
/// they belong to. `None`: the dedupe never touches such an operation.
pub(super) fn generated_key(origin: &TxOrigin) -> Option<(String, Option<&str>)> {
    match origin {
        TxOrigin::Import { exec_id } => Some((format!("import:{exec_id}"), None)),
        // An automation is a workflow now and kept its id: both
        // kinds of origin name the same planned moment alike.
        TxOrigin::Automation {
            automation_id: maker,
            planned_at,
        } => Some((format!("auto:{maker}:{planned_at}"), None)),
        // The planned moment first: an old peer's copy of the same
        // moment has no key, and both must meet.
        TxOrigin::Workflow {
            workflow_id: maker,
            run_id,
            planned_at: Some(planned_at),
            ..
        } => Some((format!("auto:{maker}:{planned_at}"), Some(run_id))),
        TxOrigin::Workflow {
            run_id,
            run_key: Some(run_key),
            ..
        } => Some((format!("run:{run_key}"), Some(run_id))),
        _ => None,
    }
}

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
    /// Mark the records of `keys` (`transactions:<id>`, `reminders:<id>`)
    /// that the dedupe could remove again. Others are left as they are.
    pub(super) fn mark_kept(&mut self, keys: &[String]) {
        let wanted: HashSet<&str> = keys.iter().map(String::as_str).collect();
        for tx in &mut self.doc.transactions {
            let meta = tx.meta_mut();
            if wanted.contains(format!("transactions:{}", meta.id).as_str())
                && generated_key(&meta.origin).is_some()
            {
                meta.keep = true;
            }
        }
        for reminder in &mut self.doc.reminders {
            if wanted.contains(format!("reminders:{}", reminder.id).as_str())
                && reminder_identity(reminder).is_some()
            {
                reminder.keep = true;
            }
        }
    }

    /// «Allow merging»: clear the mark, so the next sync may fold the record
    /// into its twin again. A local edit like any other.
    pub fn allow_merging(&mut self, target: MergeTarget, id: &str) -> Result<()> {
        let kept = match target {
            MergeTarget::Transaction => self
                .doc
                .transactions
                .iter_mut()
                .find(|tx| tx.meta().id == id)
                .map(|tx| &mut tx.meta_mut().keep),
            MergeTarget::Reminder => self
                .doc
                .reminders
                .iter_mut()
                .find(|reminder| reminder.id == id)
                .map(|reminder| &mut reminder.keep),
        };
        let Some(kept) = kept else {
            return Err(Error::NotFound(format!("record {id}")));
        };
        if !*kept {
            return Ok(());
        }
        *kept = false;
        self.persist()
    }
}
