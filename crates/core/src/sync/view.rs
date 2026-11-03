//! The vault document as sync sees it: records under `collection:key`, each
//! split into fields that merge on their own.
//!
//! Works on the serialized form (`serde_json::Value`) so one piece of code
//! covers every collection; the document is rebuilt with serde afterwards,
//! which also refuses a record that does not fit the schema.

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::flatten::{Order, join_arrays, split_array};
use super::registry::{COLLECTIONS, Collection, Rule, find};
use super::state::RecordStamps;

/// `collection:key` — the collection name has no `:`, the key may.
pub fn full_key(coll: &str, key: &str) -> String {
    format!("{coll}:{key}")
}

pub fn split_key(full: &str) -> Option<(&'static Collection, &str)> {
    let (name, key) = full.split_once(':')?;
    Some((find(name)?, key))
}

/// The one key of an externally tagged enum value, with its body.
fn variant(value: &Value) -> Option<(&String, &Value)> {
    let map = value.as_object()?;
    if map.len() != 1 {
        return None;
    }
    map.iter().next().filter(|(_, body)| body.is_object())
}

pub(super) fn lookup<'doc>(value: &'doc Value, path: &str) -> Option<&'doc Value> {
    path.split('.').try_fold(value, |node, step| {
        if step == "*" {
            variant(node).map(|(_, body)| body)
        } else {
            node.get(step)
        }
    })
}

fn key_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The key of an item of a split array (`Collection.flatten_by`).
/// An empty path means the item itself is the key (an array of strings).
pub(super) fn item_key(item: &Value, path: &str) -> Option<String> {
    if path.is_empty() {
        return Some(key_text(item));
    }
    if path.contains('+') {
        let parts: Option<Vec<String>> = path
            .split('+')
            .map(|part| lookup(item, part).map(key_text))
            .collect();
        return parts.map(|parts| parts.join("|"));
    }
    lookup(item, path).map(key_text)
}

/// [`item_key`] for tests outside this module.
#[cfg(test)]
pub fn item_key_for_tests(item: &Value, path: &str) -> Option<String> {
    item_key(item, path)
}

fn strip_item_field(items: &mut [Value], field: &str) {
    for item in items {
        if let Value::Object(map) = item {
            map.remove(field);
        }
    }
}

/// The form of a field value that counts as "changed" (`Collection.recomputed`):
/// the entry `head.<key>`, or the whole array `head`, without the item field
/// every device recomputes.
pub fn comparable<'doc>(coll: &Collection, name: &str, value: &'doc Value) -> Cow<'doc, Value> {
    for (head, field) in coll.recomputed {
        if name == *head {
            if let Value::Array(items) = value {
                let mut copy = items.clone();
                strip_item_field(&mut copy, field);
                return Cow::Owned(Value::Array(copy));
            }
        } else if name
            .strip_prefix(head)
            .is_some_and(|rest| rest.starts_with('.'))
            && let Value::Object(map) = value
            && map.contains_key(*field)
        {
            let mut copy = map.clone();
            copy.remove(*field);
            return Cow::Owned(Value::Object(copy));
        }
    }
    Cow::Borrowed(value)
}

/// A whole record in its comparable form (see [`comparable`]).
pub fn comparable_record<'doc>(coll: &Collection, value: &'doc Value) -> Cow<'doc, Value> {
    if coll.recomputed.is_empty() {
        return Cow::Borrowed(value);
    }
    let mut copy = value.clone();
    let body = match (tagged(coll), &mut copy) {
        (true, Value::Object(map)) if map.len() == 1 => map.values_mut().next(),
        _ => Some(&mut copy),
    };
    if let Some(body) = body {
        for (head, field) in coll.recomputed {
            if let Some(Value::Array(items)) = body.get_mut(*head) {
                strip_item_field(items, field);
            }
        }
    }
    Cow::Owned(copy)
}

/// The record's key inside its collection; `None` for a record without one.
pub fn record_key(coll: &Collection, value: &Value) -> Option<String> {
    if coll.key.is_empty() {
        return Some(key_text(value));
    }
    let parts: Option<Vec<String>> = coll
        .key
        .iter()
        .map(|path| lookup(value, path).map(key_text))
        .collect();
    parts.map(|parts| parts.join("|"))
}

fn tagged(coll: &Collection) -> bool {
    coll.key.first().is_some_and(|path| path.starts_with("*."))
}

/// A record split into fields; `tag` is the enum variant, when tagged.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields {
    pub tag: Option<String>,
    pub fields: BTreeMap<String, Value>,
}

/// Field name of a record that is not an object (a plain string).
pub const WHOLE: &str = "";

pub fn split(coll: &Collection, value: &Value) -> Fields {
    if coll.whole {
        return Fields {
            tag: None,
            fields: BTreeMap::from([(WHOLE.to_string(), value.clone())]),
        };
    }
    let (tag, body) = match (tagged(coll), variant(value)) {
        (true, Some((tag, body))) => (Some(tag.clone()), body),
        _ => (None, value),
    };
    let mut fields = BTreeMap::new();
    match body {
        Value::Object(map) => {
            for (name, field) in map {
                if coll.skip.contains(&name.as_str()) {
                    continue;
                }
                match (field, coll.item_key(name)) {
                    (Value::Object(inner), _) if coll.flatten.contains(&name.as_str()) => {
                        for (sub, sub_value) in inner {
                            fields.insert(format!("{name}.{sub}"), sub_value.clone());
                        }
                    }
                    (Value::Array(items), Some(path)) => match split_array(name, path, items) {
                        Some(entries) => {
                            // An empty array has no entry to say it exists.
                            if entries.is_empty() {
                                fields.insert(name.clone(), field.clone());
                            }
                            fields.extend(entries);
                        }
                        None => {
                            fields.insert(name.clone(), field.clone());
                        }
                    },
                    _ => {
                        fields.insert(name.clone(), field.clone());
                    }
                }
            }
        }
        other => {
            fields.insert(WHOLE.to_string(), other.clone());
        }
    }
    split_nested_arrays(coll, &mut fields);
    Fields { tag, fields }
}

/// Arrays inside an object that is itself split one level (`scope` ->
/// `scope.store_ids`): the head is a dotted name, and the array splits into
/// `scope.store_ids.<key>` entries after the object has split.
fn split_nested_arrays(coll: &Collection, fields: &mut BTreeMap<String, Value>) {
    for (head, path) in coll
        .flatten_by
        .iter()
        .filter(|(head, _)| head.contains('.'))
    {
        let Some(Value::Array(items)) = fields.get(*head) else {
            continue;
        };
        let Some(entries) = split_array(head, path, items) else {
            continue;
        };
        // An empty array keeps its head: the marker of "nothing left".
        if !entries.is_empty() {
            fields.remove(*head);
        }
        fields.extend(entries);
    }
}

/// `(head, key)` when `name` is the entry `head.<key>` of a split array.
fn array_entry<'name>(coll: &Collection, name: &'name str) -> Option<(&'static str, &'name str)> {
    coll.flatten_by.iter().find_map(|(head, _)| {
        name.strip_prefix(head)
            .and_then(|rest| rest.strip_prefix('.'))
            .map(|key| (*head, key))
    })
}

pub(super) fn insert_path(body: &mut Map<String, Value>, path: &str, value: Value) {
    match path.split_once('.') {
        None => {
            body.insert(path.to_string(), value);
        }
        Some((first, rest)) => {
            let slot = body
                .entry(first.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(inner) = slot {
                insert_path(inner, rest, value);
            }
        }
    }
}

pub fn join(coll: &Collection, fields: &Fields) -> Value {
    join_with(coll, fields, &Order::default())
}

/// [`join`] with the order the items of split arrays had on the devices the
/// fields come from.
pub fn join_with(coll: &Collection, fields: &Fields, order: &Order) -> Value {
    if let Some(whole) = fields.fields.get(WHOLE)
        && fields.fields.len() == 1
    {
        return whole.clone();
    }
    let mut body = Map::new();
    let mut arrays: BTreeMap<&str, BTreeMap<String, Value>> = BTreeMap::new();
    for (name, value) in &fields.fields {
        if let Some((head, key)) = array_entry(coll, name) {
            arrays
                .entry(head)
                .or_default()
                .insert(key.to_string(), value.clone());
            continue;
        }
        match name.split_once('.') {
            Some((head, sub)) if coll.flatten.contains(&head) => {
                let slot = body
                    .entry(head.to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Value::Object(inner) = slot {
                    inner.insert(sub.to_string(), value.clone());
                }
            }
            _ => {
                body.insert(name.clone(), value.clone());
            }
        }
    }
    join_arrays(coll, arrays, order, &mut body);
    // A nested array with no entry and no marker still has to exist when its
    // parent does: the typed document may require it.
    for (head, _) in coll
        .flatten_by
        .iter()
        .filter(|(head, _)| head.contains('.'))
    {
        if let Some((parent, last)) = head.rsplit_once('.')
            && let Some(Value::Object(inner)) = lookup(&Value::Object(body.clone()), parent)
            && !inner.contains_key(last)
        {
            insert_path(&mut body, head, Value::Array(Vec::new()));
        }
    }
    match &fields.tag {
        Some(tag) => {
            let mut outer = Map::new();
            outer.insert(tag.clone(), Value::Object(body));
            Value::Object(outer)
        }
        None => Value::Object(body),
    }
}

/// The record as it travels: device-only fields dropped.
pub fn shareable(coll: &Collection, value: &Value) -> Value {
    if coll.skip.is_empty() {
        return value.clone();
    }
    let stamps = RecordStamps::default();
    join_with(
        coll,
        &split(coll, value),
        &Order::new(coll, &[value], &stamps),
    )
}

/// Every synced record of a serialized document, by full key.
pub fn records(doc: &Value) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for coll in COLLECTIONS {
        let Some(part) = doc.get(coll.name) else {
            continue;
        };
        if coll.rule == Rule::Single {
            out.insert(full_key(coll.name, ""), shareable(coll, part));
            continue;
        }
        for item in part.as_array().into_iter().flatten() {
            // A repeated key: the first record wins, as in `read` and `write`.
            if let Some(key) = record_key(coll, item) {
                out.entry(full_key(coll.name, &key))
                    .or_insert_with(|| item.clone());
            }
        }
    }
    out
}

/// One record of a serialized document.
pub fn read(doc: &Value, full: &str) -> Option<Value> {
    let (coll, key) = split_key(full)?;
    let part = doc.get(coll.name)?;
    if coll.rule == Rule::Single {
        return Some(shareable(coll, part));
    }
    part.as_array()?
        .iter()
        .find(|item| record_key(coll, item).as_deref() == Some(key))
        .cloned()
}

/// Put a record into a serialized document (`None` removes it). A single
/// object keeps the device-only fields it had.
pub fn write(doc: &mut Value, full: &str, value: Option<&Value>) {
    let Some((coll, key)) = split_key(full) else {
        return;
    };
    let Some(root) = doc.as_object_mut() else {
        return;
    };
    if coll.rule == Rule::Single && coll.whole {
        root.insert(coll.name.to_string(), value.cloned().unwrap_or(Value::Null));
        return;
    }
    if coll.rule == Rule::Single {
        let Some(Value::Object(incoming)) = value else {
            return;
        };
        let slot = root
            .entry(coll.name.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(current) = slot {
            current.retain(|name, _| coll.skip.contains(&name.as_str()));
            for (name, field) in incoming {
                if !coll.skip.contains(&name.as_str()) {
                    current.insert(name.clone(), field.clone());
                }
            }
        }
        return;
    }
    let slot = root
        .entry(coll.name.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Value::Array(items) = slot else {
        return;
    };
    let position = items
        .iter()
        .position(|item| record_key(coll, item).as_deref() == Some(key));
    match (position, value) {
        (Some(index), Some(value)) => items[index] = value.clone(),
        (Some(index), None) => {
            items.remove(index);
        }
        (None, Some(value)) => items.push(value.clone()),
        (None, None) => {}
    }
}

/// Order a log by its time field and keep the newest `cap`; keep the newest
/// `cap` of a capped union.
pub fn trim(doc: &mut Value) {
    for coll in COLLECTIONS {
        let (cap, order) = match coll.rule {
            Rule::Log { cap, order } => (cap, Some(order)),
            Rule::Union { cap: Some(cap) } => (cap, None),
            _ => continue,
        };
        let Some(Value::Array(items)) = doc.get_mut(coll.name) else {
            continue;
        };
        if let Some(order) = order {
            // Equal times fall back to the key: array order differs per
            // device, and every device must keep the same records at the cap.
            items.sort_by(|left, right| {
                let time = |item: &Value| {
                    item.get(order)
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                };
                time(left)
                    .cmp(&time(right))
                    .then_with(|| record_key(coll, left).cmp(&record_key(coll, right)))
            });
        }
        if items.len() > cap {
            let extra = items.len() - cap;
            items.drain(..extra);
        }
    }
}

/// Longest label or field value shown.
const SHOWN_CHARS: usize = 80;

fn clip(text: &str) -> String {
    text.chars().take(SHOWN_CHARS).collect()
}

/// `side_job` -> `Side job`: a category key is a slug of its name, so it
/// reads well once humanized.
fn humanize(key: &str) -> String {
    let spaced = key.replace(['_', '-'], " ");
    let mut chars = spaced.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// An identifier (uuid, hash, `a|b` composite) rather than words: never shown.
fn looks_opaque(text: &str) -> bool {
    if text.contains(char::is_whitespace) {
        return false;
    }
    if text.contains(['|', ':']) {
        return true;
    }
    let digits = text.chars().filter(char::is_ascii_digit).count();
    text.len() >= 8 && (digits * 4 >= text.len() || text.contains('-') && digits > 0)
}

/// Tag of an externally tagged record and its body.
fn body_of<'doc>(coll: &Collection, value: &'doc Value) -> (Option<&'doc str>, &'doc Value) {
    match (tagged(coll), variant(value)) {
        (true, Some((tag, body))) => (Some(tag.as_str()), body),
        _ => (None, value),
    }
}

fn text_at(body: &Value, path: &str) -> Option<String> {
    match lookup(body, path)? {
        Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        _ => None,
    }
}

/// `2026-11-03` of an RFC3339 time.
fn day_of(stamp: &str) -> &str {
    stamp.get(..10).unwrap_or(stamp)
}

/// Code or symbol of the asset a record counts in, lowercase.
pub fn money_unit(body: &Value) -> Option<String> {
    ["currency", "asset.symbol", "asset.name"]
        .iter()
        .find_map(|path| text_at(body, path))
        .map(|unit| unit.to_lowercase())
}

fn transaction_label(tag: Option<&str>, body: &Value) -> String {
    let what = text_at(body, "meta.comment").or_else(|| {
        lookup(body, "meta.categories")
            .and_then(Value::as_array)
            .and_then(|list| list.iter().find_map(Value::as_str))
            .map(humanize)
    });
    let when = text_at(body, "meta.timestamp").map(|stamp| day_of(&stamp).to_string());
    let how_much = text_at(body, "amount").map(|amount| {
        let sign = match tag {
            Some("payment" | "asset_sell") if !amount.starts_with('-') => "-",
            Some("topup") if !amount.starts_with('-') => "+",
            _ => "",
        };
        let unit = money_unit(body).map(|unit| format!(" {}", unit.to_uppercase()));
        format!("{sign}{amount}{}", unit.unwrap_or_default())
    });
    let parts: Vec<String> = [what.map(|text| clip(&text)), when, how_much]
        .into_iter()
        .flatten()
        .collect();
    parts.join(", ")
}

/// The body of a record (the enum variant's content for a tagged one).
pub fn body_of_value<'doc>(coll: &Collection, value: &'doc Value) -> &'doc Value {
    body_of(coll, value).1
}

/// A short human label of a record, for the list of what a sync changes.
/// Never the raw key of a record that has only an id: that comes out empty
/// and the window names the collection instead.
pub fn label(coll: &Collection, value: &Value) -> String {
    // A credential is never a label; the window shows the collection name.
    if coll.secret {
        return String::new();
    }
    let (tag, body) = body_of(coll, value);
    if coll.name == "transactions" {
        return transaction_label(tag, body);
    }
    if coll.name == "inflation_points" {
        return [
            text_at(body, "currency").map(|code| code.to_uppercase()),
            text_at(body, "month"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    }
    for path in ["name", "title", "text", "pattern", "meta.comment", "date"] {
        // A credential field is never a label.
        if coll.secret_fields.contains(&path) {
            continue;
        }
        if let Some(text) = text_at(body, path) {
            return clip(&text);
        }
    }
    // A key made of words (a category's slug) reads fine once humanized.
    record_key(coll, value)
        .filter(|key| !key.is_empty() && !looks_opaque(key))
        .map(|key| clip(&humanize(&key)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn transactions_split_into_variant_and_meta_fields() {
        let coll = find("transactions").expect("rule");
        let tx = json!({"topup": {"meta": {"id": "t1", "comment": "hi"}, "amount": "5"}});
        assert_eq!(record_key(coll, &tx).as_deref(), Some("t1"));
        let fields = split(coll, &tx);
        assert_eq!(fields.tag.as_deref(), Some("topup"));
        assert_eq!(fields.fields.get("meta.comment"), Some(&json!("hi")));
        assert_eq!(join(coll, &fields), tx);
    }

    #[test]
    fn composite_and_whole_keys() {
        let coll = find("fx_cache").expect("rule");
        let point = json!({"currency": "eur", "ts": "2026-01-01", "per_usd": "0.9"});
        assert_eq!(record_key(coll, &point).as_deref(), Some("eur|2026-01-01"));
        let discarded = find("discarded_imports").expect("rule");
        assert_eq!(record_key(discarded, &json!("x1")).as_deref(), Some("x1"));
        assert_eq!(
            join(discarded, &split(discarded, &json!("x1"))),
            json!("x1")
        );
    }

    #[test]
    fn settings_travel_without_device_fields_and_keep_them_on_write() {
        let mut doc = json!({"settings": {"minimize_to_tray": false, "savings_rate": "10"}});
        let shared = read(&doc, "settings:").expect("settings");
        assert_eq!(shared, json!({"savings_rate": "10"}));
        write(
            &mut doc,
            "settings:",
            Some(&json!({"savings_rate": "20", "minimize_to_tray": true})),
        );
        assert_eq!(
            doc["settings"],
            json!({"minimize_to_tray": false, "savings_rate": "20"})
        );
    }

    #[test]
    fn write_adds_replaces_and_removes() {
        let mut doc = json!({"stores": [{"id": "a", "name": "A"}]});
        write(&mut doc, "stores:b", Some(&json!({"id": "b", "name": "B"})));
        write(
            &mut doc,
            "stores:a",
            Some(&json!({"id": "a", "name": "A2"})),
        );
        write(&mut doc, "stores:b", None);
        assert_eq!(doc["stores"], json!([{"id": "a", "name": "A2"}]));
    }

    #[test]
    fn logs_are_ordered_and_capped() {
        let cap = crate::model::notify::MAX_NOTICE_LOG;
        let items: Vec<Value> = (0..cap + 3)
            .rev()
            .map(|index| json!({"key": format!("k{index}"), "delivered_at": format!("{index:05}")}))
            .collect();
        let mut doc = json!({ "notification_log": items });
        trim(&mut doc);
        let log = doc["notification_log"].as_array().expect("log");
        assert_eq!(log.len(), cap);
        assert_eq!(log[0]["delivered_at"], json!("00003"));
    }
}
