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
        /// Add a line without saving (the caller saves right after).
        pub(crate) fn journal_push(&mut self, peer_name: &str, summary: &SyncSummary, outcome: SyncJournalOutcome, batch_id: Option<String>)
            => crate::usecases::sync::sync_journal::journal_push;
        /// Note a batch that was not applied (declined, failed) and save.
        pub fn sync_journal_record(&mut self, peer_name: &str, summary: &SyncSummary, outcome: SyncJournalOutcome) -> Result<()>
            => crate::usecases::sync::sync_journal::sync_journal_record;
        /// One-time cleanup on open: batches and journal lines written before
        /// secret collections were hidden keep no label or value of them.
        /// `true` when something changed (the caller saves only then).
        pub(crate) fn scrub_sync_secrets(&mut self) -> bool
            => crate::usecases::sync::sync_journal::scrub_sync_secrets;
        /// The journal, newest first.
        pub fn sync_journal(&self) -> Vec<SyncJournalEntry>
            => crate::usecases::sync::sync_journal::sync_journal;
    }
    // forward:end
}
