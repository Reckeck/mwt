//! Change tracking without touching the vault's mutations: every save
//! compares each record's fields with what the previous save left and
//! stamps what changed. A record that disappeared gets a tombstone.

use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};

use serde_json::Value;

use super::registry::Rule;
use super::stamp::{Clock, Stamp};
use super::state::{RecordStamps, SyncState};
use super::view::{records, split, split_key};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Print {
    tag: Option<String>,
    fields: BTreeMap<String, u64>,
}

fn hash_value(value: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.to_string().hash(&mut hasher);
    hasher.finish()
}

fn prints(doc: &Value) -> HashMap<String, Print> {
    records(doc)
        .into_iter()
        .filter_map(|(key, value)| {
            let (coll, _) = split_key(&key)?;
            let fields = split(coll, &value);
            let print = Print {
                tag: fields.tag,
                fields: fields
                    .fields
                    .iter()
                    .map(|(name, field)| (name.clone(), hash_value(field)))
                    .collect(),
            };
            Some((key, print))
        })
        .collect()
}

/// What the last save looked like, per record and field.
#[derive(Debug, Clone, Default)]
pub struct Tracker {
    prints: HashMap<String, Print>,
}

impl Tracker {
    pub fn capture(doc: &Value) -> Self {
        Self {
            prints: prints(doc),
        }
    }

    /// Forget the differences without stamping them: after a merge, whose
    /// stamps came with the records.
    pub fn rebase(&mut self, doc: &Value) {
        self.prints = prints(doc);
    }

    /// Stamp what changed since the last capture into `state`; returns the
    /// keys that changed (to send to devices in a live session).
    pub fn stamp(&mut self, doc: &Value, state: &mut SyncState, clock: &mut Clock) -> Vec<String> {
        let now = prints(doc);
        let mut stamp: Option<Stamp> = None;
        let mut next = || stamp.get_or_insert_with(|| clock.tick()).clone();
        let mut changed = Vec::new();

        for (key, print) in &now {
            let Some((coll, _)) = split_key(key) else {
                continue;
            };
            let tracked = !matches!(coll.rule, Rule::Union { .. });
            match self.prints.get(key) {
                Some(old) if old == print => continue,
                Some(old) if tracked && old.tag == print.tag => {
                    let at = next();
                    let entry = state.stamps.entry(key.clone()).or_default();
                    for (name, hash) in &print.fields {
                        if old.fields.get(name) != Some(hash) {
                            entry.fields.insert(name.clone(), at.clone());
                        }
                    }
                    for name in old.fields.keys() {
                        if !print.fields.contains_key(name) {
                            entry.fields.insert(name.clone(), at.clone());
                        }
                    }
                }
                _ if tracked => {
                    state
                        .stamps
                        .insert(key.clone(), RecordStamps::created(next()));
                    state.tombstones.remove(key);
                }
                _ => {}
            }
            changed.push(key.clone());
        }

        for key in self.prints.keys() {
            if now.contains_key(key) {
                continue;
            }
            let Some((coll, _)) = split_key(key) else {
                continue;
            };
            match coll.rule {
                Rule::Records | Rule::Single => {
                    state.stamps.remove(key);
                    state.tombstones.insert(key.clone(), next());
                }
                Rule::Log { .. } => {
                    state.stamps.remove(key);
                }
                Rule::Union { .. } => {}
            }
            changed.push(key.clone());
        }
        self.prints = now;
        changed
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn edits_stamp_fields_and_removals_leave_tombstones() {
        let mut doc =
            json!({"stores": [{"id": "a", "name": "A"}], "notification_log": [{"key": "n"}]});
        let mut tracker = Tracker::capture(&doc);
        let mut state = SyncState::default();
        let mut clock = Clock::new("dev1");

        doc["stores"][0]["name"] = json!("B");
        let changed = tracker.stamp(&doc, &mut state, &mut clock);
        assert_eq!(changed, vec!["stores:a".to_string()]);
        let stamps = &state.stamps["stores:a"];
        assert!(stamps.fields.contains_key("name"));
        assert!(!stamps.fields.contains_key("id"));

        doc["stores"] = json!([]);
        doc["notification_log"] = json!([]);
        tracker.stamp(&doc, &mut state, &mut clock);
        assert!(state.tombstones.contains_key("stores:a"));
        assert!(!state.stamps.contains_key("stores:a"));
        // Trimming a log is not a deletion.
        assert!(!state.tombstones.contains_key("notification_log:n"));

        doc["stores"] = json!([{"id": "a", "name": "again"}]);
        tracker.stamp(&doc, &mut state, &mut clock);
        assert!(!state.tombstones.contains_key("stores:a"));
        assert!(state.stamps["stores:a"].fields.is_empty());
    }

    #[test]
    fn nothing_changed_stamps_nothing() {
        let doc =
            json!({"stores": [{"id": "a"}], "price_cache": [{"asset_key": "btc", "ts": "1"}]});
        let mut tracker = Tracker::capture(&doc);
        let mut state = SyncState::default();
        assert!(
            tracker
                .stamp(&doc, &mut state, &mut Clock::new("d"))
                .is_empty()
        );
        assert!(state.stamps.is_empty());
    }
}
