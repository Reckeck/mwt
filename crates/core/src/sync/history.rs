//! The batches this vault kept and the transfers it made, as the history screen lists
//! them (docs/sync.md). Pure reads over the document.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{SyncBatch, SyncSummary};
use crate::model::notify::NoticeRecord;
use crate::model::vault::{VaultDoc, iso_to_millis};

/// A batch just applied, as the UI shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct AppliedBatch {
    pub id: String,
    pub peer_name: String,
    pub applied_at: String,
    pub summary: SyncSummary,
    #[serde(default)]
    pub undone_at: Option<String>,
    /// Not a batch: the vault was handed to this device from here.
    #[serde(default)]
    pub transferred_to: Option<String>,
    /// Keys of records left out because they do not fit this build's schema.
    #[serde(default)]
    pub skipped: Vec<String>,
}

/// A kept batch as the UI shows it.
pub(crate) fn applied_of(batch: &SyncBatch) -> AppliedBatch {
    AppliedBatch {
        id: batch.id.clone(),
        peer_name: batch.peer_name.clone(),
        applied_at: batch.applied_at.clone(),
        summary: batch.summary.clone(),
        undone_at: batch.undone_at.clone(),
        transferred_to: None,
        skipped: Vec::new(),
    }
}

/// Keys a kept batch changed (to pass on to the other devices).
pub fn batch_keys(doc: &VaultDoc, batch_id: &str) -> Vec<String> {
    doc.sync
        .batches
        .iter()
        .find(|batch| batch.id == batch_id)
        .map(|batch| {
            batch
                .entries
                .iter()
                .map(|entry| entry.key.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Sorted distinct collections a kept batch touched, the dedupe and the
/// repair deletions included (they are batch entries too); the summary of
/// the batch does not count those, so the `vault-synced` event names them.
pub fn batch_collections(doc: &VaultDoc, batch_id: &str) -> Vec<String> {
    batch_keys(doc, batch_id)
        .iter()
        .filter_map(|key| key.split_once(':').map(|(collection, _)| collection))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Notices a kept batch added (to show them on this device). A notice the
/// batch only updated (read on the other device) was shown before.
pub fn batch_notices(doc: &VaultDoc, batch_id: &str) -> Vec<NoticeRecord> {
    let keys: BTreeSet<&str> = doc
        .sync
        .batches
        .iter()
        .find(|batch| batch.id == batch_id)
        .into_iter()
        .flat_map(|batch| batch.entries.iter())
        .filter(|entry| entry.before.value.is_none())
        .filter_map(|entry| entry.key.strip_prefix("notification_log:"))
        .collect();
    doc.notification_log
        .iter()
        .filter(|record| keys.contains(record.key.as_str()))
        .cloned()
        .collect()
}

/// The batches still kept and the transfers to new devices, newest first.
pub fn history(doc: &VaultDoc) -> Vec<AppliedBatch> {
    let mut history: Vec<AppliedBatch> = doc.sync.batches.iter().map(applied_of).collect();
    history.extend(doc.sync.transfers.iter().map(|transfer| AppliedBatch {
        id: transfer.id.clone(),
        peer_name: transfer.device_name.clone(),
        applied_at: transfer.at.clone(),
        summary: SyncSummary::default(),
        undone_at: None,
        transferred_to: Some(transfer.device_name.clone()),
        skipped: Vec::new(),
    }));
    history.sort_by_key(|item| std::cmp::Reverse(iso_to_millis(&item.applied_at)));
    history
}
