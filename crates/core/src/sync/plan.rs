//! What a batch from another device would change here — computed before
//! anything is applied, so the user can see it (and so undo knows what to
//! put back).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::fields::changed_fields;
use super::merge::merge;
use super::registry::Collection;
use super::state::RecordState;
use super::view::{label, split_key};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    #[default]
    Added,
    Updated,
    Deleted,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct ChangeItem {
    pub kind: ChangeKind,
    /// Readable name of the record (never its raw key); may be empty.
    pub label: String,
    /// The record's key inside its collection, for identity.
    #[serde(default)]
    pub key: String,
    /// What an updated record changes, field by field.
    #[serde(default)]
    pub fields: Vec<FieldChange>,
}

/// One user-meaningful field of an updated record, as display-ready text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct FieldChange {
    /// Dotted field name (`meta.comment`); the UI maps it to a label.
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
    /// Set when the values are amounts of this currency or asset: the UI
    /// shows them with `<Money>`.
    #[serde(default)]
    pub money_currency: Option<String>,
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

    /// Blank the labels and values of secret collections (summaries written
    /// before they were hidden). `true` when something changed.
    pub fn scrub_secrets(&mut self) -> bool {
        let mut changed = false;
        for changes in &mut self.collections {
            if !super::registry::is_secret(&changes.collection) {
                continue;
            }
            for item in &mut changes.items {
                if !item.label.is_empty() {
                    item.label.clear();
                    changed = true;
                }
                if !item.fields.is_empty() {
                    item.fields.clear();
                    changed = true;
                }
            }
        }
        for field in &mut self.overwritten {
            let whole = super::registry::is_secret(&field.collection);
            if !whole && !super::registry::is_secret_field(&field.collection, &field.field) {
                continue;
            }
            if whole && !field.label.is_empty() {
                field.label.clear();
                changed = true;
            }
            for text in [&mut field.before, &mut field.after] {
                if text != MASK {
                    *text = MASK.to_string();
                    changed = true;
                }
            }
        }
        changed
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

/// Shown instead of a value of a secret collection.
pub const MASK: &str = "•••";

fn short(coll: &Collection, field: &str, value: &Value) -> String {
    if coll.secret || coll.secret_fields.contains(&field) {
        return MASK.to_string();
    }
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
        let Some((coll, inner)) = split_key(key) else {
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
        // Only stamps moved (the value is the same): nothing to show. A
        // cache point applies silently.
        if merged.state.value != before.value && !coll.cache {
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
                let fields = match (&before.value, &merged.state.value) {
                    (Some(old), Some(new)) => changed_fields(coll, old, new),
                    _ => Vec::new(),
                };
                group.items.push(ChangeItem {
                    kind,
                    label: shown.clone(),
                    key: inner.to_string(),
                    fields,
                });
            }
            out.summary.total += 1;
        }
        for overwrite in merged.overwrote {
            out.summary.overwritten.push(OverwrittenField {
                collection: coll.name.to_string(),
                label: shown.clone(),
                field: overwrite.field.clone(),
                before: short(coll, &overwrite.field, &overwrite.before),
                after: short(coll, &overwrite.field, &overwrite.after),
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::sync::stamp::Stamp;
    use crate::sync::state::RecordStamps;

    fn live(value: Value, field: Option<(&str, u64)>) -> RecordState {
        let mut stamps = RecordStamps::created(Stamp::new(1, 0, "a"));
        if let Some((name, wall)) = field {
            stamps
                .fields
                .insert(name.to_string(), Stamp::new(wall, 0, "a"));
        }
        RecordState {
            value: Some(value),
            stamps: Some(stamps),
            tombstone: None,
        }
    }

    /// The plan of one incoming record against the local one.
    fn plan_of(key: &str, local: RecordState, remote: RecordState) -> SyncPlan {
        let incoming = BTreeMap::from([(key.to_string(), remote)]);
        plan(&incoming, |_| local.clone())
    }

    #[test]
    fn a_cache_collection_is_applied_but_never_shown() {
        let point = json!({"currency": "chf", "per_usd": "0.9", "ts": "2026-11-03T10:00:00Z"});
        let key = "fx_cache:chf|2026-11-03T10:00:00Z";
        let planned = plan_of(key, RecordState::default(), live(point, None));
        assert_eq!(planned.summary.total, 0);
        assert!(planned.summary.collections.is_empty());
        assert!(planned.summary.overwritten.is_empty());
        assert!(
            planned.changes.contains_key(key),
            "the point must still apply"
        );
    }

    fn assert_no_secret(summary: &SyncSummary, secrets: &[&str]) {
        let text = serde_json::to_string(summary).expect("summary");
        for secret in secrets {
            assert!(!text.contains(secret), "{secret} leaked into {text}");
        }
    }

    #[test]
    fn an_exchange_change_shows_no_api_secret() {
        let old =
            json!({"bybit": [{"store_id": "s", "api_key": "OLDKEY", "api_secret": "OLDSECRET"}]});
        let new =
            json!({"bybit": [{"store_id": "s", "api_key": "NEWKEY", "api_secret": "NEWSECRET"}]});
        let secrets = ["OLDKEY", "OLDSECRET", "NEWKEY", "NEWSECRET"];
        // Added.
        let added = plan_of(
            "exchanges:",
            RecordState::default(),
            live(new.clone(), None),
        );
        assert_eq!(added.summary.total, 1);
        assert_no_secret(&added.summary, &secrets);
        // Updated over an edit made here: the overwritten preview.
        let updated = plan_of(
            "exchanges:",
            live(old, Some(("bybit", 5))),
            live(new, Some(("bybit", 9))),
        );
        assert_eq!(updated.summary.overwritten.len(), 1);
        assert_eq!(updated.summary.overwritten[0].before, "•••");
        assert_eq!(updated.summary.overwritten[0].after, "•••");
        assert_no_secret(&updated.summary, &secrets);
    }

    #[test]
    fn an_attachment_key_change_shows_no_sealing_key() {
        let old = json!({"id": "att1", "key": "OLDSEALINGKEY"});
        let new = json!({"id": "att1", "key": "NEWSEALINGKEY"});
        let secrets = ["OLDSEALINGKEY", "NEWSEALINGKEY"];
        let added = plan_of(
            "attachment_keys:att1",
            RecordState::default(),
            live(new.clone(), None),
        );
        assert_eq!(added.summary.total, 1);
        assert_no_secret(&added.summary, &secrets);
        let updated = plan_of(
            "attachment_keys:att1",
            live(old, Some(("key", 5))),
            live(new, Some(("key", 9))),
        );
        assert_eq!(updated.summary.overwritten.len(), 1);
        assert_no_secret(&updated.summary, &secrets);
    }

    #[test]
    fn a_data_source_url_is_masked_but_its_name_shows() {
        let old = json!({"id": "d1", "name": "Rate", "url": "https://x.test/?token=OLDTOKEN"});
        let new = json!({"id": "d1", "name": "Rate", "url": "https://x.test/?token=NEWTOKEN"});
        let secrets = ["OLDTOKEN", "NEWTOKEN"];
        let added = plan_of(
            "data_sources:d1",
            RecordState::default(),
            live(new.clone(), None),
        );
        assert_eq!(added.summary.collections[0].items[0].label, "Rate");
        assert_no_secret(&added.summary, &secrets);
        let updated = plan_of(
            "data_sources:d1",
            live(old.clone(), Some(("url", 5))),
            live(new.clone(), Some(("url", 9))),
        );
        assert_eq!(updated.summary.overwritten[0].field, "url");
        assert_eq!(updated.summary.overwritten[0].before, "•••");
        assert_eq!(updated.summary.overwritten[0].after, "•••");
        assert_eq!(updated.summary.overwritten[0].label, "Rate");
        assert_no_secret(&updated.summary, &secrets);
        // Another field of the same record shows as usual.
        let renamed_old =
            json!({"id": "d1", "name": "Mine", "url": "https://x.test/?token=OLDTOKEN"});
        let renamed_new =
            json!({"id": "d1", "name": "Theirs", "url": "https://x.test/?token=OLDTOKEN"});
        let renamed = plan_of(
            "data_sources:d1",
            live(renamed_old, Some(("name", 5))),
            live(renamed_new, Some(("name", 9))),
        );
        assert_eq!(renamed.summary.overwritten[0].before, "Mine");
        assert_eq!(renamed.summary.overwritten[0].after, "Theirs");
        assert_no_secret(&renamed.summary, &secrets);
        // The journal line holds no secret either.
        let entry = crate::sync::journal::SyncJournalEntry::of(
            "Phone",
            &updated.summary,
            crate::sync::journal::SyncJournalOutcome::Applied,
        );
        let text = serde_json::to_string(&entry).expect("entry");
        assert!(!text.contains("TOKEN"), "{text}");
    }

    #[test]
    fn a_workflow_var_value_is_masked_but_its_name_shows() {
        let old =
            json!({"scope": "global", "name": "limit", "value": "OLDVALUE", "updated_at": "a"});
        let new =
            json!({"scope": "global", "name": "limit", "value": "NEWVALUE", "updated_at": "a"});
        let secrets = ["OLDVALUE", "NEWVALUE"];
        let added = plan_of(
            "workflow_vars:global|limit",
            RecordState::default(),
            live(new.clone(), None),
        );
        assert_eq!(added.summary.collections[0].items[0].label, "limit");
        assert_no_secret(&added.summary, &secrets);
        let updated = plan_of(
            "workflow_vars:global|limit",
            live(old, Some(("value", 5))),
            live(new, Some(("value", 9))),
        );
        assert_eq!(updated.summary.overwritten[0].before, "•••");
        assert_eq!(updated.summary.overwritten[0].after, "•••");
        assert_no_secret(&updated.summary, &secrets);
    }

    #[test]
    fn the_scrub_masks_old_field_previews_and_is_idempotent() {
        let mut summary = SyncSummary {
            total: 1,
            collections: Vec::new(),
            overwritten: vec![
                OverwrittenField {
                    collection: "data_sources".into(),
                    label: "Rate".into(),
                    field: "url".into(),
                    before: "https://x.test/?token=OLDTOKEN".into(),
                    after: "https://x.test/?token=NEWTOKEN".into(),
                },
                OverwrittenField {
                    collection: "data_sources".into(),
                    label: "Rate".into(),
                    field: "name".into(),
                    before: "Mine".into(),
                    after: "Theirs".into(),
                },
            ],
        };
        assert!(summary.scrub_secrets());
        assert_eq!(summary.overwritten[0].before, "•••");
        assert_eq!(summary.overwritten[0].after, "•••");
        assert_eq!(summary.overwritten[1].before, "Mine");
        assert_eq!(summary.overwritten[1].label, "Rate");
        assert!(!summary.scrub_secrets());
    }

    #[test]
    fn an_ordinary_collection_still_shows_labels_and_values() {
        let old = json!({"id": "s", "name": "Mine"});
        let new = json!({"id": "s", "name": "Theirs"});
        let updated = plan_of(
            "stores:s",
            live(old, Some(("name", 5))),
            live(new, Some(("name", 9))),
        );
        assert_eq!(updated.summary.overwritten[0].before, "Mine");
        assert_eq!(updated.summary.overwritten[0].label, "Theirs");
    }
}
