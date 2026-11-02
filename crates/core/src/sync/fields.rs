//! The fields an edited record changes, as the question and the history show
//! them: top-level, user-meaningful, with display-ready text.

use std::collections::BTreeSet;
use std::str::FromStr;

use serde_json::Value;

use super::plan::{FieldChange, MASK};
use super::registry::Collection;
use super::view::{body_of_value, money_unit, split};

/// Most fields listed for one record.
pub const MAX_FIELDS: usize = 12;

/// Longest value shown.
const SHOWN_CHARS: usize = 80;

/// Last path steps that hold an amount of the record's currency or asset.
const MONEY_FIELDS: &[&str] = &[
    "amount",
    "limit",
    "balance",
    "price",
    "unit_price",
    "target",
    "planned",
];

/// Bookkeeping that means nothing to the user.
const INTERNAL: &[&str] = &[
    "updated_at",
    "created_at",
    "version",
    "origin",
    "meta.origin",
];

fn is_internal(coll: &Collection, field: &str) -> bool {
    let last = field.rsplit('.').next().unwrap_or(field);
    field.is_empty()
        || last == "id"
        || last.ends_with("_id")
        || last.ends_with("_ids")
        || INTERNAL.contains(&field)
        || coll.key.contains(&field)
}

fn is_money(field: &str, value: &str) -> bool {
    let last = field.rsplit('.').next().unwrap_or(field);
    (MONEY_FIELDS.contains(&last) || last.ends_with("_amount"))
        && rust_decimal::Decimal::from_str(value).is_ok()
}

/// How a value reads: `Some(None)` is "nothing", `None` is "not worth
/// listing" (a nested structure).
fn show(value: Option<&Value>) -> Option<Option<String>> {
    let clip = |text: &str| text.chars().take(SHOWN_CHARS).collect::<String>();
    match value {
        None | Some(Value::Null) => Some(None),
        Some(Value::String(text)) => Some(Some(clip(text))),
        Some(Value::Bool(flag)) => Some(Some(flag.to_string())),
        Some(Value::Number(number)) => Some(Some(number.to_string())),
        Some(Value::Array(items)) => {
            let words: Option<Vec<&str>> = items.iter().map(Value::as_str).collect();
            words.map(|words| Some(clip(&words.join(", "))))
        }
        Some(Value::Object(map)) => ["name", "symbol", "title"]
            .iter()
            .find_map(|key| map.get(*key).and_then(Value::as_str))
            .map(|text| Some(clip(text))),
    }
}

/// What an update changes, field by field; empty for a credential collection.
pub fn changed_fields(coll: &Collection, before: &Value, after: &Value) -> Vec<FieldChange> {
    if coll.secret {
        return Vec::new();
    }
    let old = split(coll, before);
    let new = split(coll, after);
    let names: BTreeSet<&String> = old.fields.keys().chain(new.fields.keys()).collect();
    let unit =
        money_unit(body_of_value(coll, after)).or_else(|| money_unit(body_of_value(coll, before)));
    let mut out = Vec::new();
    for name in names {
        if is_internal(coll, name) {
            continue;
        }
        let (old_value, new_value) = (old.fields.get(name), new.fields.get(name));
        if old_value == new_value {
            continue;
        }
        let (Some(old_text), Some(new_text)) = (show(old_value), show(new_value)) else {
            continue;
        };
        if old_text == new_text {
            continue;
        }
        if coll.secret_fields.contains(&name.as_str()) {
            out.push(FieldChange {
                field: name.clone(),
                before: old_text.map(|_| MASK.to_string()),
                after: new_text.map(|_| MASK.to_string()),
                money_currency: None,
            });
            continue;
        }
        let money = [&old_text, &new_text]
            .iter()
            .all(|text| text.as_deref().is_none_or(|text| is_money(name, text)));
        out.push(FieldChange {
            field: name.clone(),
            before: old_text,
            after: new_text,
            money_currency: if money { unit.clone() } else { None },
        });
        if out.len() == MAX_FIELDS {
            break;
        }
    }
    out
}
