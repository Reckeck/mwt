//! Merge of one record between two devices.
//!
//! The result depends only on the two states, never on which side computes
//! it, so both devices end up with the same record: per field the later
//! stamp wins; equal stamps (old data both copies had before sync existed)
//! fall back to comparing the values, which is just as deterministic.

use serde_json::Value;

use super::derived;
use super::flatten::{
    Order, cannot_express, edited_here, entry_stamp, is_entry, settle_heads, settle_orphans,
};
use super::registry::{Collection, Rule};
use super::state::{RecordStamps, RecordState};
use super::view::{Fields, comparable, is_device_field, join_with, split};

pub use super::flatten::with_legacy_heads;

/// A field the merge took from the other device although this one had
/// edited it too: shown in the summary as "overwritten".
#[derive(Debug, Clone, PartialEq)]
pub struct Overwrite {
    pub field: String,
    pub before: Value,
    pub after: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    pub state: RecordState,
    pub overwrote: Vec<Overwrite>,
    /// An external copy only (`merge_external`): some field differs and
    /// neither side has a stamp for it, so the pick is arbitrary.
    pub unstamped: bool,
}

/// The name an overwritten change of the kind of a record is listed under.
pub const KIND_FIELD: &str = "kind";

fn tag_value(fields: &Fields) -> Value {
    fields.tag.clone().map_or(Value::Null, Value::String)
}

/// `true` when the remote side wins a tie-free comparison or, on a tie, has
/// the larger value. Symmetric: swapping the sides flips the answer.
fn remote_wins(
    local: &super::stamp::Stamp,
    remote: &super::stamp::Stamp,
    local_value: Option<&Value>,
    remote_value: Option<&Value>,
) -> bool {
    if remote != local {
        return remote > local;
    }
    let canon = |value: Option<&Value>| value.map(Value::to_string).unwrap_or_default();
    canon(remote_value) > canon(local_value)
}

/// Whether two values of a field are the same edit: a recomputed item field
/// (`Collection.recomputed`) does not count.
fn same_edit(coll: &Collection, name: &str, left: Option<&Value>, right: Option<&Value>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => comparable(coll, name, left) == comparable(coll, name, right),
        (left, right) => left == right,
    }
}

/// [`merge_seen`] without knowledge of what the other device has seen: every
/// edit of this device counts.
pub fn merge(coll: &Collection, local: &RecordState, remote: &RecordState) -> Merged {
    merge_seen(coll, local, remote, None)
}

/// Whether an edit of this device made with `stamp` is news to the device the
/// record comes from. `seen` is the newest stamp of this device's clock that
/// device already has (`SyncState.seen_by`); without it every edit is news.
/// A stamp another device made is not an edit of this one.
fn unseen_edit(stamp: &super::stamp::Stamp, seen: Option<&super::stamp::Stamp>) -> bool {
    seen.is_none_or(|seen| stamp.device == seen.device && stamp > seen)
}

/// Merge `remote` into `local`; `seen` limits "overwritten" to what the
/// device `remote` comes from had not seen.
pub fn merge_seen(
    coll: &Collection,
    local: &RecordState,
    remote: &RecordState,
    seen: Option<&super::stamp::Stamp>,
) -> Merged {
    merge_core(coll, local, remote, seen, false)
}

/// [`merge`] for a record of an external copy of the vault. The value picked
/// is the same; what differs is the report: a field that differs with no
/// stamp on either side (two copies with no shared history) is listed as
/// overwritten when the other copy's value wins, and flagged in `unstamped`.
pub fn merge_external(coll: &Collection, local: &RecordState, remote: &RecordState) -> Merged {
    merge_core(coll, local, remote, None, true)
}

fn merge_core(
    coll: &Collection,
    local: &RecordState,
    remote: &RecordState,
    seen: Option<&super::stamp::Stamp>,
    external: bool,
) -> Merged {
    let unchanged = |state: &RecordState| Merged {
        state: state.clone(),
        overwrote: Vec::new(),
        unstamped: false,
    };
    if let Rule::Union { .. } = coll.rule {
        return unchanged(if local.value.is_some() { local } else { remote });
    }
    match (&local.value, &remote.value) {
        (Some(mine), Some(theirs)) => {
            merge_fields(coll, local, remote, [mine, theirs], seen, external)
        }
        (Some(_), None) => unchanged(&resolve_deletion(local, remote)),
        (None, Some(_)) => unchanged(&resolve_deletion(remote, local)),
        (None, None) => {
            let tombstone = local.tombstone.clone().max(remote.tombstone.clone());
            unchanged(&RecordState {
                value: None,
                stamps: None,
                tombstone,
            })
        }
    }
}

/// A live record against a side that has it deleted (or never had it).
fn resolve_deletion(alive: &RecordState, other: &RecordState) -> RecordState {
    let changed = alive
        .stamps
        .as_ref()
        .map(|stamps| stamps.latest().clone())
        .unwrap_or_default();
    match &other.tombstone {
        Some(deleted) if *deleted > changed => RecordState {
            value: None,
            stamps: None,
            tombstone: Some(deleted.clone()),
        },
        _ => RecordState {
            value: alive.value.clone(),
            stamps: alive.stamps.clone(),
            tombstone: None,
        },
    }
}

fn merge_fields(
    coll: &Collection,
    local: &RecordState,
    remote: &RecordState,
    [mine, theirs]: [&Value; 2],
    seen: Option<&super::stamp::Stamp>,
    external: bool,
) -> Merged {
    let local_stamps = local.stamps.clone().unwrap_or_default();
    let remote_stamps = remote.stamps.clone().unwrap_or_default();
    let local_fields = split(coll, mine);
    let remote_fields = split(coll, theirs);

    if local_fields.tag != remote_fields.tag {
        // A different kind of record altogether: the later one as a whole.
        let take_remote = remote_wins(
            local_stamps.latest(),
            remote_stamps.latest(),
            Some(mine),
            Some(theirs),
        );
        let winner = if take_remote { remote } else { local };
        // The kind this device had is gone: that is an overwrite when this
        // device itself changed the record (edited a field, or made the new
        // kind: a kind change is a fresh record with a later `created`).
        let edited_here = (local_stamps.latest() > &local_stamps.created
            || local_stamps.created > remote_stamps.created)
            && unseen_edit(local_stamps.latest(), seen);
        let overwrote = if take_remote && edited_here {
            vec![Overwrite {
                field: KIND_FIELD.to_string(),
                before: tag_value(&local_fields),
                after: tag_value(&remote_fields),
            }]
        } else {
            Vec::new()
        };
        return Merged {
            state: RecordState {
                value: winner.value.clone(),
                stamps: winner.stamps.clone(),
                tombstone: None,
            },
            overwrote,
            unstamped: false,
        };
    }

    let created = local_stamps
        .created
        .clone()
        .min(remote_stamps.created.clone());
    let mut stamps = RecordStamps::created(created.clone());
    let mut merged = Fields {
        tag: local_fields.tag.clone(),
        fields: Default::default(),
    };
    let mut overwrote = Vec::new();
    let mut unstamped_conflict = false;
    // Entries that vanished keep their stamp: it is what removes them on a
    // device that still has them.
    let stamped_entries = |stamps: &'_ RecordStamps| {
        stamps
            .fields
            .keys()
            .filter(|name| is_entry(coll, name) && !is_device_field(coll, name))
            .cloned()
            .collect::<Vec<String>>()
    };
    // Whether a derived field (`allocated`) is settled from the entries after
    // the merge: some side knows the array it is derived from.
    let derives = coll.derived.iter().any(|(_, head)| {
        derived::knows_array(&local_fields.fields, head)
            || derived::knows_array(&remote_fields.fields, head)
    });
    let names: std::collections::BTreeSet<String> = local_fields
        .fields
        .keys()
        .chain(remote_fields.fields.keys())
        .cloned()
        .chain(stamped_entries(&local_stamps))
        .chain(stamped_entries(&remote_stamps))
        .collect();
    for name in &names {
        let local_value = local_fields.fields.get(name);
        let remote_value = remote_fields.fields.get(name);
        // A whole record has one stamp: the newest of anything written to it
        // (an older peer stamps its fields one by one).
        let unstamped =
            !local_stamps.fields.contains_key(name) && !remote_stamps.fields.contains_key(name);
        let (local_stamp, remote_stamp) = if coll.whole {
            (local_stamps.latest(), remote_stamps.latest())
        } else if unstamped && same_edit(coll, name, local_value, remote_value) {
            // The same value and no stamp of its own on either side: a floor
            // would only move a stamp, not the record.
            (local_stamps.field(name), remote_stamps.field(name))
        } else {
            (
                entry_stamp(coll, &local_stamps, name),
                entry_stamp(coll, &remote_stamps, name),
            )
        };
        // A whole field stamped by an older device says nothing about
        // entries it does not list: those stay.
        // The same for an array a field is derived from: a side that does not
        // know the array lists no entry and removes none.
        let unknown_array = |fields: &std::collections::BTreeMap<String, Value>| {
            derived::source_of(coll, name).is_some_and(|head| !derived::knows_array(fields, head))
        };
        let take_remote = if remote_value.is_none() && unknown_array(&remote_fields.fields) {
            false
        } else if local_value.is_none() && unknown_array(&local_fields.fields) {
            true
        } else if remote_value.is_none()
            && cannot_express(coll, &remote_fields.fields, &remote_stamps, name)
        {
            false
        } else if local_value.is_none()
            && cannot_express(coll, &local_fields.fields, &local_stamps, name)
        {
            true
        } else {
            remote_wins(local_stamp, remote_stamp, local_value, remote_value)
        };
        let (value, stamp) = if take_remote {
            (remote_value, remote_stamp)
        } else {
            (local_value, local_stamp)
        };
        let differs = !same_edit(coll, name, local_value, remote_value)
            && !(derives && derived::is_derived(coll, name));
        // No shared history: nobody can say which side changed the field.
        // A whole record is ordered by its newest stamp, so only a tie there
        // is a guess.
        let guessed = external
            && differs
            && unstamped
            && !is_device_field(coll, name)
            && (!coll.whole || local_stamp == remote_stamp);
        unstamped_conflict |= guessed;
        if take_remote
            && differs
            && (guessed
                || (edited_here(coll, &local_stamps, name) && unseen_edit(local_stamp, seen)))
        {
            overwrote.push(Overwrite {
                field: name.clone(),
                before: local_value.cloned().unwrap_or(Value::Null),
                after: remote_value.cloned().unwrap_or(Value::Null),
            });
        }
        if let Some(value) = value {
            merged.fields.insert(name.clone(), value.clone());
        }
        if *stamp != created {
            stamps.fields.insert(name.clone(), stamp.clone());
        }
    }
    if derives {
        let local_side = derived::Side {
            fields: &local_fields.fields,
            stamps: &local_stamps,
        };
        let remote_side = derived::Side {
            fields: &remote_fields.fields,
            stamps: &remote_stamps,
        };
        derived::settle(
            coll,
            &local_side,
            &remote_side,
            &mut merged.fields,
            &mut stamps,
        );
    }
    settle_orphans(
        coll,
        &mut merged.fields,
        &mut stamps,
        &[&local_fields.fields, &remote_fields.fields],
    );
    settle_heads(coll, &mut merged.fields, &stamps);
    let order = Order::new(coll, &[mine, theirs], &stamps);
    let mut value = join_with(coll, &merged, &order);
    keep_device_fields(coll, &mut value, mine);
    Merged {
        state: RecordState {
            value: Some(value),
            stamps: Some(stamps),
            tombstone: None,
        },
        overwrote,
        unstamped: unstamped_conflict,
    }
}

/// Fields that belong to the device stay as they are here; a merge never
/// takes them from the other side.
fn keep_device_fields(coll: &Collection, merged: &mut Value, local: &Value) {
    if coll.skip.is_empty() {
        return;
    }
    super::view::keep_nested_device_fields(coll, merged, local);
    let (Value::Object(body), Value::Object(mine)) = (merged, local) else {
        return;
    };
    for name in coll.skip {
        if let Some(value) = mine.get(*name) {
            body.insert((*name).to_string(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::registry::find;
    use super::super::stamp::Stamp;
    use super::*;

    fn at(wall: u64, device: &str) -> Stamp {
        Stamp::new(wall, 0, device)
    }

    fn state(value: Value, created: Stamp, fields: &[(&str, Stamp)]) -> RecordState {
        RecordState {
            value: Some(value),
            stamps: Some(RecordStamps {
                created,
                fields: fields
                    .iter()
                    .map(|(name, stamp)| ((*name).to_string(), stamp.clone()))
                    .collect(),
            }),
            tombstone: None,
        }
    }

    fn deleted(stamp: Stamp) -> RecordState {
        RecordState {
            value: None,
            stamps: None,
            tombstone: Some(stamp),
        }
    }

    fn both_ways(coll: &Collection, left: &RecordState, right: &RecordState) -> RecordState {
        let there = merge(coll, left, right).state;
        let back = merge(coll, right, left).state;
        assert_eq!(there, back, "merge must not depend on the side");
        there
    }

    #[test]
    fn edits_of_different_fields_both_survive() {
        let coll = find("stores").expect("rule");
        let base = at(1, "a");
        let left = state(
            json!({"id": "s", "name": "Cash 2", "archived": false}),
            base.clone(),
            &[("name", at(5, "a"))],
        );
        let right = state(
            json!({"id": "s", "name": "Cash", "archived": true}),
            base,
            &[("archived", at(6, "b"))],
        );
        let merged = both_ways(coll, &left, &right);
        assert_eq!(
            merged.value,
            Some(json!({"id": "s", "name": "Cash 2", "archived": true}))
        );
    }

    #[test]
    fn the_later_edit_of_one_field_wins_and_is_reported() {
        let coll = find("stores").expect("rule");
        let left = state(
            json!({"id": "s", "name": "Mine"}),
            at(1, "a"),
            &[("name", at(5, "a"))],
        );
        let right = state(
            json!({"id": "s", "name": "Theirs"}),
            at(1, "a"),
            &[("name", at(7, "b"))],
        );
        let merged = merge(coll, &left, &right);
        assert_eq!(
            merged.state.value,
            Some(json!({"id": "s", "name": "Theirs"}))
        );
        assert_eq!(merged.overwrote.len(), 1);
        assert_eq!(merged.overwrote[0].before, json!("Mine"));
        assert!(merge(coll, &right, &left).overwrote.is_empty());
    }

    #[test]
    fn a_later_deletion_wins_and_a_later_edit_resurrects() {
        let coll = find("stores").expect("rule");
        let alive = state(
            json!({"id": "s", "name": "x"}),
            at(1, "a"),
            &[("name", at(5, "a"))],
        );
        let gone = both_ways(coll, &alive, &deleted(at(9, "b")));
        assert_eq!(gone.value, None);
        assert_eq!(gone.tombstone, Some(at(9, "b")));
        let back = both_ways(coll, &alive, &deleted(at(3, "b")));
        assert_eq!(back.value, Some(json!({"id": "s", "name": "x"})));
        assert_eq!(back.tombstone, None);
    }

    #[test]
    fn old_copies_without_stamps_still_converge() {
        let coll = find("stores").expect("rule");
        let left = RecordState {
            value: Some(json!({"id": "s", "name": "A"})),
            ..Default::default()
        };
        let right = RecordState {
            value: Some(json!({"id": "s", "name": "B"})),
            ..Default::default()
        };
        let merged = both_ways(coll, &left, &right);
        assert_eq!(merged.value, Some(json!({"id": "s", "name": "B"})));
    }

    #[test]
    fn a_changed_transaction_kind_takes_the_later_record() {
        let coll = find("transactions").expect("rule");
        let left = state(
            json!({"topup": {"meta": {"id": "t"}, "amount": "1"}}),
            at(1, "a"),
            &[],
        );
        let right = state(
            json!({"payment": {"meta": {"id": "t"}, "amount": "2"}}),
            at(4, "b"),
            &[],
        );
        let merged = both_ways(coll, &left, &right);
        assert_eq!(merged.value, right.value);
    }

    #[test]
    fn a_lost_kind_change_is_reported_as_overwritten() {
        let coll = find("transactions").expect("rule");
        // This device turned a top-up into a payment (a new record: fresh
        // `created`); the other one edited the top-up later.
        let mine = state(
            json!({"payment": {"meta": {"id": "t"}, "amount": "1"}}),
            at(3, "a"),
            &[],
        );
        let theirs = state(
            json!({"topup": {"meta": {"id": "t"}, "amount": "2"}}),
            at(1, "a"),
            &[("topup.amount", at(5, "b"))],
        );
        let merged = merge(coll, &mine, &theirs);
        assert_eq!(merged.state.value, theirs.value);
        assert_eq!(merged.overwrote.len(), 1, "{:?}", merged.overwrote);
        assert_eq!(merged.overwrote[0].before, json!("payment"));
        assert_eq!(merged.overwrote[0].after, json!("topup"));
        // The side that won loses nothing.
        assert!(merge(coll, &theirs, &mine).overwrote.is_empty());
    }

    #[test]
    fn a_lost_field_edit_against_a_kind_change_is_reported_as_overwritten() {
        let coll = find("transactions").expect("rule");
        let mine = state(
            json!({"topup": {"meta": {"id": "t"}, "amount": "2"}}),
            at(1, "a"),
            &[("topup.amount", at(4, "a"))],
        );
        let theirs = state(
            json!({"payment": {"meta": {"id": "t"}, "amount": "1"}}),
            at(6, "b"),
            &[],
        );
        let merged = merge(coll, &mine, &theirs);
        assert_eq!(merged.overwrote.len(), 1, "{:?}", merged.overwrote);
        assert_eq!(merged.overwrote[0].before, json!("topup"));
    }

    #[test]
    fn an_untouched_record_that_changes_kind_elsewhere_is_no_overwrite() {
        let coll = find("transactions").expect("rule");
        let mine = state(
            json!({"topup": {"meta": {"id": "t"}, "amount": "1"}}),
            at(1, "a"),
            &[],
        );
        let theirs = state(
            json!({"payment": {"meta": {"id": "t"}, "amount": "1"}}),
            at(6, "b"),
            &[],
        );
        assert!(merge(coll, &mine, &theirs).overwrote.is_empty());
    }

    #[test]
    fn an_edit_up_to_the_seen_stamp_is_no_overwrite_a_later_one_is() {
        let coll = find("stores").expect("rule");
        let mine = state(
            json!({"id": "s", "name": "Mine"}),
            at(1, "a"),
            &[("name", at(5, "a"))],
        );
        let theirs = state(
            json!({"id": "s", "name": "Theirs"}),
            at(1, "a"),
            &[("name", at(7, "b"))],
        );
        let overwrites =
            |seen: Option<Stamp>| merge_seen(coll, &mine, &theirs, seen.as_ref()).overwrote;
        assert_eq!(overwrites(None).len(), 1);
        assert!(overwrites(Some(at(5, "a"))).is_empty());
        assert!(overwrites(Some(at(6, "a"))).is_empty());
        assert_eq!(overwrites(Some(at(4, "a"))).len(), 1);
    }

    #[test]
    fn a_stamp_of_another_device_is_not_an_edit_of_this_one() {
        let coll = find("stores").expect("rule");
        let mine = state(
            json!({"id": "s", "name": "Mine"}),
            at(1, "a"),
            &[("name", at(5, "c"))],
        );
        let theirs = state(
            json!({"id": "s", "name": "Theirs"}),
            at(1, "a"),
            &[("name", at(7, "b"))],
        );
        assert!(
            merge_seen(coll, &mine, &theirs, Some(&at(2, "a")))
                .overwrote
                .is_empty()
        );
    }

    #[test]
    fn a_kind_change_the_peer_saw_is_no_overwrite() {
        let coll = find("transactions").expect("rule");
        let mine = state(
            json!({"payment": {"meta": {"id": "t"}, "amount": "1"}}),
            at(3, "a"),
            &[],
        );
        let theirs = state(
            json!({"topup": {"meta": {"id": "t"}, "amount": "2"}}),
            at(1, "a"),
            &[("topup.amount", at(5, "b"))],
        );
        assert!(
            merge_seen(coll, &mine, &theirs, Some(&at(3, "a")))
                .overwrote
                .is_empty()
        );
        assert_eq!(
            merge_seen(coll, &mine, &theirs, Some(&at(2, "a")))
                .overwrote
                .len(),
            1
        );
    }

    #[test]
    fn union_keeps_what_either_side_has() {
        let coll = find("price_cache").expect("rule");
        let point = RecordState {
            value: Some(json!({"asset_key": "btc", "ts": "1"})),
            ..Default::default()
        };
        assert_eq!(merge(coll, &RecordState::default(), &point).state, point);
        assert_eq!(merge(coll, &point, &RecordState::default()).state, point);
    }
}
