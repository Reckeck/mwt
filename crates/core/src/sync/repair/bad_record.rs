//! A record the typed document refuses (a schema this build does not know,
//! a damaged value) must not end the session: it is found, left out, and told
//! once in the journal while the good records of the batch apply.

use serde_json::Value;

use crate::model::vault::VaultDoc;
use crate::sync::journal::SyncJournalEntry;
use crate::sync::plan::{PlannedChange, SyncPlan};
use crate::sync::view;

/// Whether the document still parses with `changes` written into `base`.
fn parses(base: &Value, changes: &[(&String, &PlannedChange)]) -> bool {
    let mut probe = base.clone();
    for (key, change) in changes {
        view::write(&mut probe, key, change.after.value.as_ref());
    }
    view::trim(&mut probe);
    serde_json::from_value::<VaultDoc>(probe).is_ok()
}

/// The keys that make the document unparsable, by halving: a few bad records
/// cost a few parses, not one per record.
fn bad_of(base: &Value, changes: &[(&String, &PlannedChange)]) -> Vec<String> {
    if changes.is_empty() || parses(base, changes) {
        return Vec::new();
    }
    if let [(key, _)] = changes {
        return vec![(*key).clone()];
    }
    let (first, second) = changes.split_at(changes.len() / 2);
    let mut bad = bad_of(base, first);
    bad.extend(bad_of(base, second));
    bad
}

/// Keys of `plan` whose record the typed document refuses, `base` being
/// the document before the batch. Empty when the failure is not down to
/// a single record.
pub fn unparsable_keys(base: &Value, plan: &SyncPlan) -> Vec<String> {
    let changes: Vec<(&String, &PlannedChange)> = plan.changes.iter().collect();
    bad_of(base, &changes)
}

/// Whether the journal already holds this line: the peer offers a skipped
/// record again every session, and the journal must not fill with one line
/// per session.
pub fn journal_has(journal: &[SyncJournalEntry], line: &SyncJournalEntry) -> bool {
    journal.iter().any(|entry| {
        entry.outcome == line.outcome
            && entry.peer_name == line.peer_name
            && entry.counts == line.counts
            && entry.label == line.label
    })
}
