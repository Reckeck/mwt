//! The batches this vault kept and the transfers it made, as the history screen lists
//! them (docs/sync.md). Pure reads over the document. Also the balance diff of a
//! sync preview: which stores a batch moves.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::{SyncBatch, SyncSummary};
use crate::Amount;
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

/// One store whose balance differs between two sides of a sync preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreChange {
    pub id: String,
    pub name: String,
    pub before: Amount,
    pub after: Amount,
}

/// One side of the diff: the USD value of each store and the rate that turns
/// USD into the display currency on that side.
pub struct StoreValues<'a> {
    pub usd: &'a HashMap<String, Amount>,
    pub fx: Amount,
}

/// The stores whose balance differs between `before` and `after`, in the
/// display `currency`, sorted by name then id. A store missing on one side
/// counts as zero there; `names` gives the label of each store id.
pub fn store_changes(
    before: &StoreValues<'_>,
    after: &StoreValues<'_>,
    names: &HashMap<&str, &str>,
) -> Vec<StoreChange> {
    let mut stores: Vec<StoreChange> = Vec::new();
    for id in before.usd.keys().chain(after.usd.keys()) {
        if stores.iter().any(|store| &store.id == id) {
            continue;
        }
        let old = before.usd.get(id).copied().unwrap_or_default() * before.fx;
        let new = after.usd.get(id).copied().unwrap_or_default() * after.fx;
        if old == new {
            continue;
        }
        stores.push(StoreChange {
            id: id.clone(),
            name: names
                .get(id.as_str())
                .copied()
                .unwrap_or_default()
                .to_string(),
            before: old,
            after: new,
        });
    }
    stores.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    stores
}

#[cfg(test)]
mod store_changes_tests {
    use super::*;

    fn amount(text: &str) -> Amount {
        Amount::parse(text).unwrap()
    }

    fn values(entries: &[(&str, &str)]) -> HashMap<String, Amount> {
        entries
            .iter()
            .map(|(id, value)| (id.to_string(), amount(value)))
            .collect()
    }

    #[test]
    fn store_changes_lists_moved_stores_sorted_by_name_and_skips_unchanged() {
        let before = values(&[("a", "10"), ("b", "5"), ("same", "7")]);
        let after = values(&[("a", "12"), ("c", "3"), ("same", "7")]);
        let names = HashMap::from([("a", "Zeta"), ("b", "Beta"), ("c", "Alpha"), ("same", "S")]);
        let one = amount("1");
        let changes = store_changes(
            &StoreValues {
                usd: &before,
                fx: one,
            },
            &StoreValues {
                usd: &after,
                fx: one,
            },
            &names,
        );
        let ids: Vec<&str> = changes.iter().map(|change| change.id.as_str()).collect();
        assert_eq!(ids, ["c", "b", "a"]);
        assert_eq!(changes[0].before, amount("0"));
        assert_eq!(changes[0].after, amount("3"));
        assert_eq!(changes[1].after, amount("0"));
        assert_eq!(changes[2].before, amount("10"));
        assert_eq!(changes[2].after, amount("12"));
    }

    #[test]
    fn store_changes_applies_each_sides_rate() {
        let usd = values(&[("a", "10")]);
        let changes = store_changes(
            &StoreValues {
                usd: &usd,
                fx: amount("1"),
            },
            &StoreValues {
                usd: &usd,
                fx: amount("2"),
            },
            &HashMap::new(),
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].before, amount("10"));
        assert_eq!(changes[0].after, amount("20"));
        assert_eq!(changes[0].name, "");
    }
}
