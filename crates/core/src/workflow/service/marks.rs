//! What a workflow remembers between passes: the marks of its triggers, where
//! it stopped reading operations, and the zone its schedules count in. Pure
//! functions over the document; a scenario commits what they change.

use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::VaultDoc;
use crate::pricing::history::unix_secs;
use crate::workflow::graph::Graph;
use crate::workflow::model::WorkflowState;

/// The state record of a workflow, made when it has none yet.
pub fn state_mut<'doc>(doc: &'doc mut VaultDoc, workflow_id: &str) -> &'doc mut WorkflowState {
    let index = match doc
        .workflow_state
        .iter()
        .position(|state| state.workflow_id == workflow_id)
    {
        Some(index) => index,
        None => {
            doc.workflow_state.push(WorkflowState {
                workflow_id: workflow_id.to_string(),
                ..WorkflowState::default()
            });
            doc.workflow_state.len() - 1
        }
    };
    &mut doc.workflow_state[index]
}

/// One mark of a workflow.
pub fn mark(doc: &VaultDoc, workflow_id: &str, key: &str) -> Option<String> {
    doc.workflow_state
        .iter()
        .find(|state| state.workflow_id == workflow_id)
        .and_then(|state| state.marks.get(key).cloned())
}

pub fn set_mark(doc: &mut VaultDoc, workflow_id: &str, key: &str, value: &str) {
    state_mut(doc, workflow_id)
        .marks
        .insert(key.to_string(), value.to_string());
}

/// The schedule marks that stand below a moment whose run key is booked, as
/// `(workflow id, mark key, moment)`: whatever moved a mark back (an undo), the
/// booked moments do not fire again, so these are to be raised.
pub fn raised_schedule_marks(doc: &VaultDoc) -> Vec<(String, String, String)> {
    let booked: Vec<&str> = doc
        .transactions
        .iter()
        .filter_map(|tx| match &tx.meta().origin {
            TxOrigin::Workflow {
                run_key: Some(key), ..
            } => Some(key.as_str()),
            _ => None,
        })
        .collect();
    let mut raised = Vec::new();
    for state in &doc.workflow_state {
        for (key, mark) in &state.marks {
            let Some(trigger_id) = key.strip_prefix("schedule:") else {
                continue;
            };
            let prefix = format!("wf:{}:{trigger_id}:", state.workflow_id);
            let newest = booked
                .iter()
                .filter_map(|run_key| run_key.strip_prefix(prefix.as_str()))
                .filter_map(|moment| unix_secs(moment).map(|secs| (secs, moment)))
                .max();
            if let Some((secs, moment)) = newest
                && unix_secs(mark).is_none_or(|current| current < secs)
            {
                raised.push((state.workflow_id.clone(), key.clone(), moment.to_string()));
            }
        }
    }
    raised
}

/// Start watching from now: schedules count from this moment, operations
/// from the next one added.
pub fn arm(doc: &mut VaultDoc, workflow_id: &str, graph: &Graph, now: i64) {
    let stamp = crate::calendar::iso(now);
    let (index, cursor) = (
        doc.transactions.len(),
        doc.transactions.last().map(|tx| tx.meta().id.clone()),
    );
    let state = state_mut(doc, workflow_id);
    state.tx_index = index;
    state.tx_cursor = cursor;
    for node in graph
        .nodes
        .iter()
        .filter(|node| node.kind == "trigger.schedule")
    {
        state
            .marks
            .insert(format!("schedule:{}", node.id), stamp.clone());
    }
}

/// Operations added since a workflow last looked, and where it is now.
pub fn new_transactions(
    doc: &VaultDoc,
    workflow_id: &str,
) -> (Vec<Transaction>, usize, Option<String>) {
    let len = doc.transactions.len();
    let last = doc.transactions.last().map(|tx| tx.meta().id.clone());
    let Some(state) = doc
        .workflow_state
        .iter()
        .find(|state| state.workflow_id == workflow_id)
    else {
        return (Vec::new(), len, last);
    };
    let start = match &state.tx_cursor {
        Some(cursor) => doc
            .transactions
            .iter()
            .position(|tx| tx.meta().id == *cursor)
            .map_or(state.tx_index.min(len), |index| index + 1),
        None => state.tx_index.min(len),
    };
    (doc.transactions[start..].to_vec(), len, last)
}

/// Everything up to `len` was looked at; what a run adds is seen next pass.
pub fn mark_seen(doc: &mut VaultDoc, workflow_id: &str, len: usize, last: Option<String>) {
    let state = state_mut(doc, workflow_id);
    state.tx_index = len;
    state.tx_cursor = last;
}

/// The zone schedule moments are computed in: the group's, shared by every
/// device. The first device that gets here writes its own zone.
pub fn schedule_zone(doc: &mut VaultDoc, device_zone: &str) -> String {
    doc.settings
        .schedule_timezone
        .get_or_insert_with(|| device_zone.to_string())
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_is_made_once_and_keeps_its_marks() {
        let mut doc = VaultDoc::default();
        set_mark(&mut doc, "wf", "schedule:t", "2026-01-01T00:00:00Z");
        set_mark(&mut doc, "wf", "threshold:u", "above");
        assert_eq!(doc.workflow_state.len(), 1);
        assert_eq!(mark(&doc, "wf", "threshold:u").as_deref(), Some("above"));
        assert_eq!(mark(&doc, "wf", "missing"), None);
        assert_eq!(mark(&doc, "other", "threshold:u"), None);
    }

    #[test]
    fn the_first_device_zone_is_the_one_kept() {
        let mut doc = VaultDoc::default();
        assert_eq!(schedule_zone(&mut doc, "Europe/Paris"), "Europe/Paris");
        assert_eq!(schedule_zone(&mut doc, "Asia/Tokyo"), "Europe/Paris");
    }

    #[test]
    fn a_workflow_without_state_sees_every_operation_as_seen() {
        let doc = VaultDoc::default();
        let (fresh, len, last) = new_transactions(&doc, "wf");
        assert!(fresh.is_empty());
        assert_eq!(len, 0);
        assert_eq!(last, None);
    }
}
