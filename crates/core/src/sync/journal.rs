//! The sync journal: one line per batch taken from another device, kept in
//! the vault for the «History» table (applied, declined, failed). A ring: the
//! newest [`MAX_JOURNAL`] lines stay. No record values, no keys: only
//! collection names, counts and, for a single change, its label.

use serde::{Deserialize, Serialize};

use super::plan::SyncSummary;

/// Lines kept.
pub const MAX_JOURNAL: usize = 200;

/// How a batch ended.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum SyncJournalOutcome {
    #[default]
    Applied,
    Declined,
    Failed,
    /// The vault was handed to the device (a transfer): nothing to apply.
    Sent,
}

/// How many records of one collection a batch touched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncJournalCount {
    pub collection: String,
    pub count: u32,
}

/// One batch, as the journal tells it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncJournalEntry {
    pub id: String,
    /// RFC3339.
    pub at: String,
    /// The device the batch came from.
    pub peer_name: String,
    pub counts: Vec<SyncJournalCount>,
    /// The record's label when the batch changed exactly one.
    #[serde(default)]
    pub label: Option<String>,
    pub outcome: SyncJournalOutcome,
    /// The kept batch behind an applied line, for "Undo" while it is kept.
    #[serde(default)]
    pub batch_id: Option<String>,
}

impl SyncJournalEntry {
    /// Drop the label of a line about a secret collection (written before
    /// they were hidden). `true` when something changed.
    pub fn scrub_secrets(&mut self) -> bool {
        let secret = self
            .counts
            .iter()
            .any(|count| super::registry::is_secret(&count.collection));
        secret && self.label.take().is_some()
    }

    /// A line for `summary`, without the id and time (the vault sets them).
    pub fn of(peer_name: &str, summary: &SyncSummary, outcome: SyncJournalOutcome) -> Self {
        let counts = summary
            .collections
            .iter()
            .map(|changes| SyncJournalCount {
                collection: changes.collection.clone(),
                count: changes.added + changes.updated + changes.deleted,
            })
            .filter(|count| count.count > 0)
            .collect();
        let label = (summary.total == 1)
            .then(|| {
                summary
                    .collections
                    .iter()
                    .find_map(|changes| changes.items.first())
                    .map(|item| item.label.clone())
            })
            .flatten()
            // A secret collection has no label: the window names the collection.
            .filter(|label| !label.is_empty());
        Self {
            id: String::new(),
            at: String::new(),
            peer_name: peer_name.to_string(),
            counts,
            label,
            outcome,
            batch_id: None,
        }
    }
}

/// Add `entry` and drop the oldest lines past the cap (oldest first in `log`).
pub fn push(log: &mut Vec<SyncJournalEntry>, entry: SyncJournalEntry) {
    log.push(entry);
    let extra = log.len().saturating_sub(MAX_JOURNAL);
    log.drain(..extra);
}

#[cfg(test)]
mod secret_tests {
    use serde_json::json;

    use super::*;
    use crate::sync::plan::plan;
    use crate::sync::state::RecordStamps;
    use crate::sync::{RecordState, stamp::Stamp};

    /// An older peer still sends its exchange keys as a record; keys are
    /// device-local now, so the record is not planned and leaves nothing.
    #[test]
    fn a_legacy_exchange_key_record_leaves_no_secret_in_the_journal() {
        let remote = RecordState {
            value: Some(json!({"bybit": [{"api_key": "JKEY", "api_secret": "JSECRET"}]})),
            stamps: Some(RecordStamps::created(Stamp::new(1, 0, "a"))),
            tombstone: None,
        };
        let incoming = std::collections::BTreeMap::from([("exchanges:".to_string(), remote)]);
        let summary = plan(&incoming, |_| RecordState::default()).summary;
        assert_eq!(summary.total, 0);
        let entry = SyncJournalEntry::of("Phone", &summary, SyncJournalOutcome::Applied);
        let text = serde_json::to_string(&entry).expect("entry");
        assert!(
            !text.contains("JKEY") && !text.contains("JSECRET"),
            "{text}"
        );
        assert!(entry.label.is_none());
    }
}
