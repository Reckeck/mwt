//! Pure reads and edits of the sync journal kept in `SyncState`: a new line with the id and time
//! given by the caller, the newest-first listing and the one-time scrub of secret labels. Reading
//! the clock and saving are in `usecases::sync::sync_journal`.

use super::SyncSummary;
use super::journal::{SyncJournalEntry, SyncJournalOutcome, push};
use super::state::SyncState;

/// Add a line about `summary` to the journal (oldest first, capped); `id` and `at` are set by the caller.
pub fn push_line(
    state: &mut SyncState,
    peer_name: &str,
    summary: &SyncSummary,
    outcome: SyncJournalOutcome,
    batch_id: Option<String>,
    (id, at): (String, String),
) {
    let mut entry = SyncJournalEntry::of(peer_name, summary, outcome);
    entry.batch_id = batch_id;
    entry.id = id;
    entry.at = at;
    push(&mut state.journal, entry);
}

/// Batches and journal lines written before secret collections were hidden keep no label or value
/// of them. `true` when something changed.
pub fn scrub_secrets(state: &mut SyncState) -> bool {
    let mut changed = false;
    for batch in &mut state.batches {
        changed |= batch.summary.scrub_secrets();
    }
    for entry in &mut state.journal {
        changed |= entry.scrub_secrets();
    }
    changed
}

/// The journal, newest first.
pub fn newest_first(state: &SyncState) -> Vec<SyncJournalEntry> {
    state.journal.iter().rev().cloned().collect()
}
