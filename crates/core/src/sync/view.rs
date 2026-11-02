//! The vault document as sync sees it: records under `collection:key`, each
//! split into fields that merge on their own.
//!
//! Works on the serialized form (`serde_json::Value`) so one piece of code
//! covers every collection; the document is rebuilt with serde afterwards,
//! which also refuses a record that does not fit the schema.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::registry::{COLLECTIONS, Collection, Rule, find};

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

fn lookup<'doc>(value: &'doc Value, path: &str) -> Option<&'doc Value> {
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
                match field {
                    Value::Object(inner) if coll.flatten.contains(&name.as_str()) => {
                        for (sub, sub_value) in inner {
                            fields.insert(format!("{name}.{sub}"), sub_value.clone());
                        }
                    }
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
    Fields { tag, fields }
}

pub fn join(coll: &Collection, fields: &Fields) -> Value {
    if let Some(whole) = fields.fields.get(WHOLE)
        && fields.fields.len() == 1
    {
        return whole.clone();
    }
    let mut body = Map::new();
    for (name, value) in &fields.fields {
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
    join(coll, &split(coll, value))
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
            if let Some(key) = record_key(coll, item) {
                out.insert(full_key(coll.name, &key), item.clone());
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
            items.sort_by(|left, right| {
                let left = left.get(order).and_then(Value::as_str).unwrap_or("");
                let right = right.get(order).and_then(Value::as_str).unwrap_or("");
                left.cmp(right)
            });
        }
        if items.len() > cap {
            let extra = items.len() - cap;
            items.drain(..extra);
        }
    }
}

/// A short human label of a record, for the list of what a sync changes.
pub fn label(coll: &Collection, value: &Value) -> String {
    // A credential is never a label; the window shows the collection name.
    if coll.secret {
        return String::new();
    }
    let body = match (tagged(coll), variant(value)) {
        (true, Some((_, body))) => body,
        _ => value,
    };
    if let Value::String(text) = body {
        return text.clone();
    }
    for path in [
        "name",
        "title",
        "text",
        "pattern",
        "meta.comment",
        "asset_key",
        "key",
        "date",
    ] {
        // A credential field is never a label.
        if coll.secret_fields.contains(&path) {
            continue;
        }
        if let Some(Value::String(text)) = lookup(body, path)
            && !text.trim().is_empty()
        {
            return text.chars().take(80).collect();
        }
    }
    record_key(coll, value)
        .unwrap_or_default()
        .chars()
        .take(80)
        .collect()
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
