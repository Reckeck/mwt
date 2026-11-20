//! Tests of the sync journal lines (`sync::sync_journal`).

use super::SyncSummary;
use super::journal::{MAX_JOURNAL, SyncJournalOutcome};
use super::state::SyncState;
use super::sync_journal::*;

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
