//! Change tracking without touching the vault's mutations: every save
//! compares each record's fields with what the previous save left and
//! stamps what changed. A record that disappeared gets a tombstone.

use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};

use serde_json::Value;

use super::registry::Rule;
use super::stamp::{Clock, MAX_FUTURE_MS, Stamp, now_ms};
use super::state::{RecordStamps, SyncState};
use super::view::{comparable, records, split, split_key};

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
                    .map(|(name, field)| (name.clone(), hash_value(&comparable(coll, name, field))))
                    .collect(),
            };
            Some((key, print))
        })
        .collect()
}

/// The stamp of a change in this save: the save's tick, or one past the
/// record's `floor` when that stamp lies ahead of the tick. A `floor` further
/// ahead than [`MAX_FUTURE_MS`] marks a poisoned record (peers would skip a
/// stamp one past it): the change is stamped now and a warning is logged.
fn stamp_for(base: &mut Option<Stamp>, clock: &mut Clock, floor: Option<&Stamp>) -> Stamp {
    stamp_for_at(base, clock, floor, now_ms(), &mut |message| {
        eprintln!("{message}");
    })
}

/// [`stamp_for`] with an explicit `now` (ms) and the sink of the warning.
fn stamp_for_at(
    base: &mut Option<Stamp>,
    clock: &mut Clock,
    floor: Option<&Stamp>,
    now: u64,
    warn: &mut dyn FnMut(&str),
) -> Stamp {
    let tick = base.get_or_insert_with(|| clock.tick_at(now)).clone();
    match floor {
        Some(existing) if existing.wall > now.saturating_add(MAX_FUTURE_MS) => {
            warn(&format!(
                "sync: a record is stamped {} ms ahead of the clock (limit {MAX_FUTURE_MS}); \
                 the edit is stamped now",
                existing.wall - now
            ));
            tick
        }
        Some(existing) => tick.max(clock.tick_after(existing)),
        None => tick,
    }
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
        // The tick of this save, taken once and only when something changed.
        let mut base: Option<Stamp> = None;
        let mut changed = Vec::new();

        for (key, print) in &now {
            let Some((coll, _)) = split_key(key) else {
                continue;
            };
            let tracked = !matches!(coll.rule, Rule::Union { .. });
            match self.prints.get(key) {
                Some(old) if old == print => continue,
                Some(old) if tracked && old.tag == print.tag => {
                    let entry = state.stamps.entry(key.clone()).or_default();
                    let floor = print
                        .fields
                        .iter()
                        .filter(|(name, hash)| old.fields.get(*name) != Some(*hash))
                        .map(|(name, _)| entry.field(name))
                        .chain(
                            old.fields
                                .keys()
                                .filter(|name| !print.fields.contains_key(*name))
                                .map(|name| entry.field(name)),
                        )
                        .max()
                        .cloned();
                    let at = stamp_for(&mut base, clock, floor.as_ref());
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
                    let at = stamp_for(&mut base, clock, state.tombstones.get(key));
                    state.stamps.insert(key.clone(), RecordStamps::created(at));
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
                    let floor = state.stamps.remove(key).map(|old| old.latest().clone());
                    let at = stamp_for(&mut base, clock, floor.as_ref());
                    state.tombstones.insert(key.clone(), at);
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

    fn stamped(doc: &Value, clock: &mut Clock, state: &mut SyncState, old: &Value) {
        Tracker::capture(old).stamp(doc, state, clock);
    }

    #[test]
    fn a_normal_edit_gets_the_current_tick() {
        let old = json!({"stores": [{"id": "a", "name": "A"}]});
        let new = json!({"stores": [{"id": "a", "name": "B"}]});
        let mut state = SyncState::default();
        state.stamps.insert(
            "stores:a".into(),
            RecordStamps::created(Stamp::new(1, 0, "x")),
        );
        let before = now_ms();
        stamped(&new, &mut Clock::new("dev1"), &mut state, &old);
        let at = state.stamps["stores:a"].field("name");
        assert!(at.wall >= before && at.wall <= now_ms());
    }

    #[test]
    fn an_edit_of_a_future_stamped_record_wins_the_merge() {
        let future = Stamp::new(now_ms() + 3_600_000, 2, "zz");
        let old = json!({"stores": [{"id": "a", "name": "A"}]});
        let new = json!({"stores": [{"id": "a", "name": "B"}]});
        let mut state = SyncState::default();
        let mut stamps = RecordStamps::created(Stamp::new(1, 0, "x"));
        stamps.fields.insert("name".into(), future.clone());
        state.stamps.insert("stores:a".into(), stamps);
        stamped(&new, &mut Clock::new("dev1"), &mut state, &old);
        assert!(*state.stamps["stores:a"].field("name") > future);
    }

    #[test]
    fn a_stamp_within_the_limit_still_gets_one_past_it() {
        let now = 1_000_000;
        let existing = Stamp::new(now + MAX_FUTURE_MS, 2, "zz");
        let mut warnings = Vec::new();
        let at = stamp_for_at(
            &mut None,
            &mut Clock::new("dev1"),
            Some(&existing),
            now,
            &mut |message| warnings.push(message.to_string()),
        );
        assert_eq!(at, Stamp::new(now + MAX_FUTURE_MS, 3, "dev1"));
        assert!(warnings.is_empty(), "an in-limit stamp warned");
    }

    #[test]
    fn a_stamp_beyond_the_limit_is_capped_to_now() {
        let now = 1_000_000;
        let existing = Stamp::new(now + MAX_FUTURE_MS + 1, 2, "zz");
        let mut warnings = Vec::new();
        let at = stamp_for_at(
            &mut None,
            &mut Clock::new("dev1"),
            Some(&existing),
            now,
            &mut |message| warnings.push(message.to_string()),
        );
        assert_eq!(at.wall, now);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("86400001 ms ahead"));
    }

    #[test]
    fn a_deletion_of_a_future_stamped_record_does_not_resurrect() {
        let future = Stamp::new(now_ms() + 3_600_000, 2, "zz");
        let old = json!({"stores": [{"id": "a", "name": "A"}]});
        let new = json!({"stores": []});
        let mut state = SyncState::default();
        let mut stamps = RecordStamps::created(Stamp::new(1, 0, "x"));
        stamps.fields.insert("name".into(), future.clone());
        state.stamps.insert("stores:a".into(), stamps);
        stamped(&new, &mut Clock::new("dev1"), &mut state, &old);
        assert!(state.tombstones["stores:a"] > future);
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
