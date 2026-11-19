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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::journal::MAX_JOURNAL;

    fn line(state: &mut SyncState, outcome: SyncJournalOutcome) {
        push_line(
            state,
            "Pixel",
            &SyncSummary::default(),
            outcome,
            None,
            ("id".into(), "2026-01-01T00:00:00Z".into()),
        );
    }

    #[test]
    fn a_line_carries_the_given_id_time_and_batch() {
        let mut state = SyncState::default();
        push_line(
            &mut state,
            "Pixel",
            &SyncSummary::default(),
            SyncJournalOutcome::Applied,
            Some("b1".into()),
            ("l1".into(), "2026-01-01T00:00:00Z".into()),
        );
        let entry = &newest_first(&state)[0];
        assert_eq!(
            (entry.id.as_str(), entry.at.as_str()),
            ("l1", "2026-01-01T00:00:00Z")
        );
        assert_eq!(entry.batch_id.as_deref(), Some("b1"));
    }

    #[test]
    fn the_listing_is_newest_first_and_capped() {
        let mut state = SyncState::default();
        for _ in 0..MAX_JOURNAL {
            line(&mut state, SyncJournalOutcome::Declined);
        }
        line(&mut state, SyncJournalOutcome::Failed);
        let log = newest_first(&state);
        assert_eq!(log.len(), MAX_JOURNAL);
        assert_eq!(log[0].outcome, SyncJournalOutcome::Failed);
    }
}
