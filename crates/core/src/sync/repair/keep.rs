//! The «keep» mark (stage M): a copy the user got back by undoing a sync that
//! had merged it away. The dedupe skips every pair that holds a marked record,
//! so the next batch does not remove the copy a second time. The mark is an
//! ordinary synced field; a plain edit leaves it, «Allow merging» clears it.

use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::model::planning::Reminder;
use crate::model::transaction::TxOrigin;
use crate::model::vault::VaultDoc;

/// What two operations must share for the dedupe to treat them as one made
/// twice (an import, an automation moment or a workflow run), and the run
/// they belong to. `None`: the dedupe never touches such an operation.
pub fn generated_key(origin: &TxOrigin) -> Option<(String, Option<&str>)> {
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

/// Mark the records of `keys` (`transactions:<id>`, `reminders:<id>`)
/// that the dedupe could remove again. Others are left as they are.
/// `reminder_identity` says what makes a reminder one per source (it lives with the
/// automatic reminders, outside this module's reach).
pub fn mark_kept(
    doc: &mut VaultDoc,
    keys: &[String],
    reminder_identity: fn(&Reminder) -> Option<String>,
) {
    let wanted: HashSet<&str> = keys.iter().map(String::as_str).collect();
    for tx in &mut doc.transactions {
        let meta = tx.meta_mut();
        if wanted.contains(format!("transactions:{}", meta.id).as_str())
            && generated_key(&meta.origin).is_some()
        {
            meta.keep = true;
        }
    }
    for reminder in &mut doc.reminders {
        if wanted.contains(format!("reminders:{}", reminder.id).as_str())
            && reminder_identity(reminder).is_some()
        {
            reminder.keep = true;
        }
    }
}

/// The kind of record the «keep» mark sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepTarget {
    Transaction,
    Reminder,
}

/// Whether the record is marked «keep»; `NotFound` when it is not there.
pub fn is_kept(doc: &VaultDoc, target: KeepTarget, id: &str) -> Result<bool> {
    let kept = match target {
        KeepTarget::Transaction => doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == id)
            .map(|tx| tx.meta().keep),
        KeepTarget::Reminder => doc
            .reminders
            .iter()
            .find(|reminder| reminder.id == id)
            .map(|reminder| reminder.keep),
    };
    kept.ok_or_else(|| Error::NotFound(format!("record {id}")))
}

/// Clear the mark, so the next sync may fold the record into its twin again.
pub fn allow_merging(doc: &mut VaultDoc, target: KeepTarget, id: &str) {
    match target {
        KeepTarget::Transaction => {
            if let Some(tx) = doc.transactions.iter_mut().find(|tx| tx.meta().id == id) {
                tx.meta_mut().keep = false;
            }
        }
        KeepTarget::Reminder => {
            if let Some(reminder) = doc.reminders.iter_mut().find(|reminder| reminder.id == id) {
                reminder.keep = false;
            }
        }
    }
}
