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
    /// The kept batch behind an applied line, for «Отменить» while it is kept.
    #[serde(default)]
    pub batch_id: Option<String>,
}

impl SyncJournalEntry {
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
            .flatten();
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
