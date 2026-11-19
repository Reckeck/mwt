//! R9: a staged exchange fill whose `exec_id` is already booked as an
//! operation or discarded is dropped from the queue.
//!
//! Two devices can queue the same fill, one confirms or discards it before it
//! has seen the other's entry; the later entry then outlives the tombstone of
//! the first. The decision (the operation, or the "discarded" mark) is already
//! made, so the entry only adds noise and a second confirmation would book a
//! duplicate. Money is untouched. The pass is silent: nothing is announced.

use std::collections::HashSet;

use super::repair_report::RepairReport;
use crate::model::vault::VaultDoc;

/// Drop the staged fills whose `exec_id` is in `decided` (the operations and
/// discards of this vault; the queue itself is not "decided").
pub(crate) fn repair_pending_imports(
    doc: &mut VaultDoc,
    decided: &HashSet<String>,
    report: &mut RepairReport,
) {
    let mut dropped: Vec<String> = doc
        .pending_imports
        .iter()
        .filter(|pending| decided.contains(&pending.exec_id))
        .map(|pending| pending.exec_id.clone())
        .collect();
    if dropped.is_empty() {
        return;
    }
    doc.pending_imports
        .retain(|pending| !decided.contains(&pending.exec_id));
    dropped.sort();
    dropped.dedup();
    report.dropped_imports = dropped;
}
