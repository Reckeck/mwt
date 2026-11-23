//! Shim of the `sync_journal` area: the line, the listing and the scrub moved to
//! `crate::sync::sync_journal`, the scenarios to `crate::usecases::sync::sync_journal` (spec
//! core-strangler, D1).

use super::UnlockedVault;
use super::forward::forward;
use crate::error::Result;
use crate::sync::SyncSummary;
use crate::sync::journal::{SyncJournalEntry, SyncJournalOutcome};

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Note a batch that was not applied (declined, failed) and save.
        pub fn sync_journal_record(&mut self, peer_name: &str, summary: &SyncSummary, outcome: SyncJournalOutcome) -> Result<()>
            => crate::usecases::sync::sync_journal::sync_journal_record;
        /// The journal, newest first.
        pub fn sync_journal(&self) -> Vec<SyncJournalEntry>
            => crate::usecases::sync::sync_journal::sync_journal;
    }
    // forward:end
}
