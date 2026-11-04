//! R9: a staged exchange fill whose `exec_id` is already booked as an
//! operation or discarded is dropped from the queue.
//!
//! Two devices can queue the same fill, one confirms or discards it before it
//! has seen the other's entry; the later entry then outlives the tombstone of
//! the first. The decision (the operation, or the "discarded" mark) is already
//! made, so the entry only adds noise and a second confirmation would book a
//! duplicate. Money is untouched. The pass is silent: nothing is announced.

use super::UnlockedVault;
use super::repair_report::RepairReport;

impl UnlockedVault {
    pub(super) fn repair_pending_imports(&mut self, report: &mut RepairReport) {
        if self.doc.pending_imports.is_empty() {
            return;
        }
        // The queue itself is not "decided": only operations and discards are.
        let decided = self.decided_import_ids();
        let mut dropped: Vec<String> = self
            .doc
            .pending_imports
            .iter()
            .filter(|pending| decided.contains(&pending.exec_id))
            .map(|pending| pending.exec_id.clone())
            .collect();
        if dropped.is_empty() {
            return;
        }
        self.doc
            .pending_imports
            .retain(|pending| !decided.contains(&pending.exec_id));
        dropped.sort();
        dropped.dedup();
        report.dropped_imports = dropped;
    }
}
