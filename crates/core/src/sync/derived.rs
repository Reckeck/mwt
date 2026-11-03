//! Fields computed from a split array (`Collection.derived`): an envelope's
//! `allocated` is the sum of its `moves`.
//!
//! Between devices that know the array the field is never an edit of its own.
//! After a merge it is re-derived from the merged entries and stamped with
//! the stamp of the newest entry (and a sender stamps it that way too, see
//! [`with_legacy_heads`](super::flatten::with_legacy_heads)), so a device that
//! recomputed it cannot look as if it had edited it.
//!
//! A device that does not know the array (an older version: its value has no
//! `moves` at all) just writes the field. That is recognised by a stamp
//! newer than anything the other side holds for the field, which is neither
//! the stamp of the whole array (the value of a new device an old one only
//! forwarded) nor older. It becomes one entry, `fix:<record id>:<stamp>`, with
//! the stamp of that write: the id comes only from the write, so the entry is
//! made once however often and by however many devices the write is seen.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::registry::Collection;
use super::stamp::Stamp;
use super::state::RecordStamps;
use crate::Amount;

/// Whether the side lists the array `head`, even as an empty one: only a
/// device that does not know the array has neither the head nor an entry.
pub fn knows_array(fields: &BTreeMap<String, Value>, head: &str) -> bool {
    let prefix = format!("{head}.");
    fields.contains_key(head) || fields.keys().any(|name| name.starts_with(&prefix))
}

/// The array a field is derived from, when `name` is one of its entries.
pub fn source_of(coll: &Collection, name: &str) -> Option<&'static str> {
    coll.derived.iter().map(|(_, head)| *head).find(|head| {
        name.strip_prefix(head)
            .is_some_and(|rest| rest.starts_with('.'))
    })
}

/// Whether `name` is a field that is derived.
pub fn is_derived(coll: &Collection, name: &str) -> bool {
    coll.derived.iter().any(|(field, _)| *field == name)
}

/// The stamp of the newest entry of `head`.
pub fn newest_entry<'stamps>(stamps: &'stamps RecordStamps, head: &str) -> Option<&'stamps Stamp> {
    let prefix = format!("{head}.");
    stamps
        .fields
        .iter()
        .filter(|(name, _)| name.starts_with(&prefix))
        .map(|(_, stamp)| stamp)
        .max()
}

fn amount_of(value: Option<&Value>) -> Option<Amount> {
    value.and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn sum_of(fields: &BTreeMap<String, Value>, head: &str) -> Option<Amount> {
    let prefix = format!("{head}.");
    let mut any = false;
    let mut total = Amount::ZERO;
    for (name, entry) in fields {
        if name.starts_with(&prefix) {
            any = true;
            total += amount_of(entry.get("delta")).unwrap_or(Amount::ZERO);
        }
    }
    any.then_some(total)
}

fn stamp_text(stamp: &Stamp) -> String {
    format!("{}-{}-{}", stamp.wall, stamp.count, stamp.device)
}

fn stamp_time(stamp: &Stamp) -> String {
    use time::format_description::well_known::Rfc3339;
    i64::try_from(stamp.wall)
        .ok()
        .and_then(|millis| {
            time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000).ok()
        })
        .and_then(|time| time.format(&Rfc3339).ok())
        .unwrap_or_default()
}

/// One side of a merge.
pub struct Side<'side> {
    pub fields: &'side BTreeMap<String, Value>,
    pub stamps: &'side RecordStamps,
}

/// A write of the derived field by a device without the array: its value and
/// stamp, when it is newer than everything `other` holds for the field and
/// is no echo of a new device's value.
fn old_write(side: &Side, other: &Side, (field, head): (&str, &str)) -> Option<(Amount, Stamp)> {
    if knows_array(side.fields, head) {
        return None;
    }
    let stamp = side.stamps.fields.get(field)?;
    // An old device that only forwards a new one's value keeps the stamp of
    // the whole array beside it.
    if side.stamps.fields.get(head) == Some(stamp) {
        return None;
    }
    let held = other
        .stamps
        .field(field)
        .max(newest_entry(other.stamps, head).unwrap_or(&other.stamps.created));
    (stamp > held).then_some(())?;
    Some((amount_of(side.fields.get(field))?, stamp.clone()))
}

/// After the entries of a merge are decided: derive the fields from them.
pub fn settle(
    coll: &Collection,
    local: &Side,
    remote: &Side,
    merged: &mut BTreeMap<String, Value>,
    stamps: &mut RecordStamps,
) {
    for &(field, head) in coll.derived {
        if !knows_array(local.fields, head) && !knows_array(remote.fields, head) {
            continue;
        }
        let id = merged
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        // A correction whose stamp is the stamp of a real entry was made from
        // the echo of a new device's value (an old device forwarded it without
        // the stamp of the array): the real entry is the truth. Its stamp
        // stays as the removal mark for devices that still hold the entry.
        let fix_prefix = format!("{head}.fix:");
        let real_stamps = |merged: &BTreeMap<String, Value>, stamps: &RecordStamps| {
            let prefix = format!("{head}.");
            merged
                .keys()
                .filter(|name| name.starts_with(&prefix) && !name.starts_with(&fix_prefix))
                .filter_map(|name| stamps.fields.get(name).cloned())
                .collect::<Vec<Stamp>>()
        };
        let echoes: Vec<String> = {
            let real = real_stamps(merged, stamps);
            merged
                .keys()
                .filter(|name| name.starts_with(&fix_prefix))
                .filter(|name| {
                    stamps
                        .fields
                        .get(*name)
                        .is_some_and(|stamp| real.contains(stamp))
                })
                .cloned()
                .collect()
        };
        for name in echoes {
            merged.remove(&name);
        }
        let real = real_stamps(merged, stamps);
        let writes = [
            old_write(local, remote, (field, head)),
            old_write(remote, local, (field, head)),
        ];
        for (target, stamp) in writes.into_iter().flatten() {
            if real.contains(&stamp) {
                continue;
            }
            let name = format!("{head}.fix:{id}:{}", stamp_text(&stamp));
            if merged.contains_key(&name) {
                continue;
            }
            let held = sum_of(merged, head).unwrap_or(Amount::ZERO);
            if target == held {
                continue;
            }
            let entry = json!({
                "id": format!("fix:{id}:{}", stamp_text(&stamp)),
                "delta": target - held,
                "at": stamp_time(&stamp),
            });
            merged.insert(name.clone(), entry);
            stamps.fields.insert(name, stamp);
        }
        // No entry at all: a device that knows the array always has its own
        // field at zero then, so a value beside no entries is an echo.
        let total = sum_of(merged, head).unwrap_or(Amount::ZERO);
        merged.insert(field.to_string(), json!(total));
        if let Some(newest) = newest_entry(stamps, head).cloned() {
            stamps.fields.insert(field.to_string(), newest);
        }
    }
}
