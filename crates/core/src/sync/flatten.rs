//! Fields that sync as separate entries: an object split one level
//! (`Collection.flatten`, `meta.<key>`) or an array split by item id
//! (`Collection.flatten_by`, `items.<id>`).
//!
//! An entry has its own stamp. A stamp of the whole field (`items`, written
//! by an older device, or left from before the split) is a floor for every
//! entry without a stamp of its own.

use std::collections::{BTreeMap, HashSet};

use serde_json::{Map, Value};

use super::registry::Collection;
use super::stamp::Stamp;
use super::state::RecordStamps;
use super::view::{body_of_value, insert_path, item_key, lookup};

/// Whether `head` is a field that `name` is an entry of (`items` of
/// `items.7`).
fn under(head: &str, name: &str) -> bool {
    name.strip_prefix(head)
        .is_some_and(|rest| rest.starts_with('.'))
}

/// The innermost head of an entry name (`scope.store_ids` of
/// `scope.store_ids.7`, where `scope` is split too), when it is one.
fn head_of(coll: &Collection, name: &str) -> Option<&'static str> {
    coll.heads()
        .filter(|head| under(head, name))
        .max_by_key(|head| head.len())
}

/// Whether `name` is an entry of a split field.
pub fn is_entry(coll: &Collection, name: &str) -> bool {
    head_of(coll, name).is_some()
}

/// The stamp of the whole field `head` when it says something of its own: an
/// older device wrote the field as one, or the field was last written empty.
/// A head stamp that only repeats the newest of its entries is what
/// [`with_legacy_heads`] adds for older receivers; it is no floor, or every
/// entry the sender never touched would look as new as its newest one.
fn whole_stamp<'stamps>(stamps: &'stamps RecordStamps, head: &str) -> Option<&'stamps Stamp> {
    let own = stamps.fields.get(head)?;
    let newest = stamps
        .fields
        .iter()
        .filter(|(name, _)| under(head, name))
        .map(|(_, stamp)| stamp)
        .max();
    (newest != Some(own)).then_some(own)
}

/// When the entry `name` last changed: its own stamp, else the stamp of the
/// whole field it sits in (a floor), else the creation of the record.
pub fn entry_stamp<'stamps>(
    coll: &Collection,
    stamps: &'stamps RecordStamps,
    name: &str,
) -> &'stamps Stamp {
    if let Some(own) = stamps.fields.get(name) {
        return own;
    }
    // Every head above the entry may carry the stamp of an older device.
    let floor = coll
        .heads()
        .filter(|head| under(head, name))
        .filter_map(|head| whole_stamp(stamps, head))
        .max();
    match floor {
        Some(floor) if *floor > stamps.created => floor,
        _ => &stamps.created,
    }
}

/// Whether the side edited the entry: its own stamp, or the stamp of a whole
/// field above it (an older device writes the field as one).
pub fn edited_here(coll: &Collection, stamps: &RecordStamps, name: &str) -> bool {
    stamps.fields.contains_key(name)
        || coll
            .heads()
            .any(|head| under(head, name) && whole_stamp(stamps, head).is_some())
}

/// A side whose whole field was stamped but that lists no value for the
/// entry (or no entries at all) cannot say the entry was removed: an older
/// device has no way to express it, so the other side's entry stays.
pub fn cannot_express(
    coll: &Collection,
    fields: &BTreeMap<String, Value>,
    stamps: &RecordStamps,
    name: &str,
) -> bool {
    let Some(head) = head_of(coll, name) else {
        return false;
    };
    let head_stamped = coll
        .heads()
        .any(|above| under(above, name) && stamps.fields.contains_key(above));
    if stamps.fields.contains_key(name) || !head_stamped {
        return false;
    }
    fields.get(head).is_none_or(is_empty_container)
}

fn is_empty_container(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

/// After the entries merged: a whole-field value left beside them (`null`, a
/// scalar, an array of unkeyed items) against the entries; the newest side
/// wins. An empty container is only the marker of "nothing left" and gives
/// way to any entry.
pub fn settle_heads(
    coll: &Collection,
    fields: &mut BTreeMap<String, Value>,
    stamps: &RecordStamps,
) {
    for head in coll.heads() {
        let Some(value) = fields.get(head) else {
            continue;
        };
        let prefix = format!("{head}.");
        let entries: Vec<String> = fields
            .keys()
            .filter(|name| name.starts_with(&prefix))
            .cloned()
            .collect();
        if entries.is_empty() {
            continue;
        }
        let head_wins = !is_empty_container(value)
            && entries
                .iter()
                .all(|name| entry_stamp(coll, stamps, name) < entry_stamp(coll, stamps, head));
        if head_wins {
            for name in entries {
                fields.remove(&name);
            }
        } else {
            fields.remove(head);
        }
    }
}

/// Entries of the array `items` of a record. `None` when an item has no key
/// and the array has to stay whole. The first item of a repeated key wins;
/// later ones are dropped.
pub fn split_array(head: &str, key_path: &str, items: &[Value]) -> Option<Vec<(String, Value)>> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for item in items {
        let key = item_key(item, key_path)?;
        if seen.insert(key.clone()) {
            entries.push((format!("{head}.{key}"), item.clone()));
        }
    }
    Some(entries)
}

/// The order items were in on the devices a merge came from.
#[derive(Default)]
pub struct Order<'doc> {
    bodies: Vec<&'doc Value>,
    stamps: Option<&'doc RecordStamps>,
}

impl<'doc> Order<'doc> {
    pub fn new(coll: &Collection, values: &[&'doc Value], stamps: &'doc RecordStamps) -> Self {
        Self {
            bodies: values
                .iter()
                .map(|value| body_of_value(coll, value))
                .collect(),
            stamps: Some(stamps),
        }
    }

    fn keys(&self, head: &str, key_path: &str) -> Vec<Vec<String>> {
        self.bodies
            .iter()
            .map(|body| {
                let mut seen = HashSet::new();
                lookup(body, head)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item_key(item, key_path))
                    .filter(|key| seen.insert(key.clone()))
                    .collect()
            })
            .collect()
    }
}

/// Join the entries of one array in a deterministic order: the sequence both
/// devices share, where they differ the entries only one side has first (the
/// older change first, then by key), then by key. The result does not
/// depend on which device computes it.
pub fn join_array(
    coll: &Collection,
    head: &str,
    entries: &BTreeMap<String, Value>,
    order: &Order,
) -> Vec<Value> {
    let Some(key_path) = coll.item_key(head) else {
        return Vec::new();
    };
    let rank = |key: &str| {
        order
            .stamps
            .map(|stamps| entry_stamp(coll, stamps, &format!("{head}.{key}")).clone())
            .unwrap_or_default()
    };
    let mut sequence: Vec<String> = Vec::new();
    for keys in order.keys(head, key_path) {
        let keys: Vec<String> = keys
            .into_iter()
            .filter(|key| entries.contains_key(key))
            .collect();
        sequence = merge_sequences(&sequence, &keys, &rank);
    }
    let placed: HashSet<&String> = sequence.iter().collect();
    let mut rest: Vec<&String> = entries.keys().filter(|key| !placed.contains(key)).collect();
    rest.sort_by(|left, right| (rank(left), left).cmp(&(rank(right), right)));
    let rest: Vec<String> = rest.into_iter().cloned().collect();
    sequence
        .iter()
        .chain(rest.iter())
        .filter_map(|key| entries.get(key).cloned())
        .collect()
}

fn merge_sequences(left: &[String], right: &[String], rank: &dyn Fn(&str) -> Stamp) -> Vec<String> {
    let in_left: HashSet<&String> = left.iter().collect();
    let in_right: HashSet<&String> = right.iter().collect();
    let mut out: Vec<String> = Vec::new();
    let mut done: HashSet<String> = HashSet::new();
    let (mut at_left, mut at_right) = (0, 0);
    loop {
        while left.get(at_left).is_some_and(|key| done.contains(key)) {
            at_left += 1;
        }
        while right.get(at_right).is_some_and(|key| done.contains(key)) {
            at_right += 1;
        }
        let pick = match (left.get(at_left), right.get(at_right)) {
            (None, None) => break,
            (Some(key), None) | (None, Some(key)) => key,
            (Some(from_left), Some(from_right)) if from_left == from_right => from_left,
            (Some(from_left), Some(from_right)) => {
                match (!in_right.contains(from_left), !in_left.contains(from_right)) {
                    (true, false) => from_left,
                    (false, true) => from_right,
                    _ => {
                        if (rank(from_left), from_left) <= (rank(from_right), from_right) {
                            from_left
                        } else {
                            from_right
                        }
                    }
                }
            }
        };
        done.insert(pick.clone());
        out.push(pick.clone());
    }
    out
}

/// What an older device reads: a stamp for each split field as a whole, the
/// newest of its entries (removed ones count). Added only to what is sent,
/// never stored.
pub fn with_legacy_heads(coll: &Collection, stamps: &mut RecordStamps) {
    for head in coll.heads() {
        let prefix = format!("{head}.");
        let newest = stamps
            .fields
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .map(|(_, stamp)| stamp)
            .chain(stamps.fields.get(head))
            .max()
            .cloned();
        if let Some(newest) = newest {
            stamps.fields.insert(head.to_string(), newest);
        }
    }
}

/// Build the body entries of the split arrays into `body`.
pub fn join_arrays(
    coll: &Collection,
    arrays: BTreeMap<&str, BTreeMap<String, Value>>,
    order: &Order,
    body: &mut Map<String, Value>,
) {
    for (head, entries) in arrays {
        insert_path(
            body,
            head,
            Value::Array(join_array(coll, head, &entries, order)),
        );
    }
}
