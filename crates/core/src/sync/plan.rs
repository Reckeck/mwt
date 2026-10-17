//! What a batch from another device would change here — computed before
//! anything is applied, so the user can see it (and so undo knows what to
//! put back).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::merge::merge;
use super::state::RecordState;
use super::view::{label, split_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Updated,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct ChangeItem {
    pub kind: ChangeKind,
    pub label: String,
}

/// Changes to one collection (`transactions`, `stores`, …).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CollectionChanges {
    pub collection: String,
    pub added: u32,
    pub updated: u32,
    pub deleted: u32,
    /// The first [`MAX_ITEMS`] changes, for the list under the summary.
    pub items: Vec<ChangeItem>,
}

/// A field edited on both devices where the other device's later edit won.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct OverwrittenField {
    pub collection: String,
    pub label: String,
    pub field: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncSummary {
    pub total: u32,
    pub collections: Vec<CollectionChanges>,
    pub overwritten: Vec<OverwrittenField>,
}

impl SyncSummary {
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
}

/// Items listed per collection; the counts cover the rest.
pub const MAX_ITEMS: usize = 50;

/// One record the batch changes: as it is here and as it will be.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedChange {
    pub before: RecordState,
    pub after: RecordState,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncPlan {
    pub summary: SyncSummary,
    /// By full key.
    pub changes: BTreeMap<String, PlannedChange>,
}

fn short(value: &Value) -> String {
    let text = match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    text.chars().take(80).collect()
}

/// Merge each incoming record with the local one (`local` gives the local
/// state of a key) and keep those that change.
pub fn plan(
    incoming: &BTreeMap<String, RecordState>,
    local: impl Fn(&str) -> RecordState,
) -> SyncPlan {
    let mut out = SyncPlan::default();
    let mut groups: BTreeMap<&'static str, CollectionChanges> = BTreeMap::new();
    for (key, remote) in incoming {
        let Some((coll, _)) = split_key(key) else {
            continue;
        };
        let before = local(key);
        let merged = merge(coll, &before, remote);
        if merged.state.value == before.value && merged.state.same_marks(&before) {
            continue;
        }
        let kind = match (&before.value, &merged.state.value) {
            (None, Some(_)) => ChangeKind::Added,
            (Some(_), None) => ChangeKind::Deleted,
            _ => ChangeKind::Updated,
        };
        let shown = merged
            .state
            .value
            .as_ref()
            .or(before.value.as_ref())
            .map(|value| label(coll, value))
            .unwrap_or_default();
        // Only stamps moved (the value is the same): nothing to show.
        if merged.state.value != before.value {
            let group = groups
                .entry(coll.name)
                .or_insert_with(|| CollectionChanges {
                    collection: coll.name.to_string(),
                    ..Default::default()
                });
            match kind {
                ChangeKind::Added => group.added += 1,
                ChangeKind::Updated => group.updated += 1,
                ChangeKind::Deleted => group.deleted += 1,
            }
            if group.items.len() < MAX_ITEMS {
                group.items.push(ChangeItem {
                    kind,
                    label: shown.clone(),
                });
            }
            out.summary.total += 1;
        }
        for overwrite in merged.overwrote {
            out.summary.overwritten.push(OverwrittenField {
                collection: coll.name.to_string(),
                label: shown.clone(),
                field: overwrite.field,
                before: short(&overwrite.before),
                after: short(&overwrite.after),
            });
        }
        out.changes.insert(
            key.clone(),
            PlannedChange {
                before,
                after: merged.state,
            },
        );
    }
    out.summary.collections = groups.into_values().collect();
    out
}
