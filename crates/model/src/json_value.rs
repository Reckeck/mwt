//! Reading JSON parameter fields as numbers and text: what a node parameter
//! or a schedule parameter holds.

use serde_json::{Map, Value};

use mwt_base::Amount;

/// A number from a field: a JSON number or a string with one.
pub fn json_number(value: &Value) -> Option<Amount> {
    match value {
        Value::Number(number) => Amount::parse(&number.to_string()).ok(),
        Value::String(text) => Amount::parse(text.trim().replace(',', ".").as_str()).ok(),
        _ => None,
    }
}

/// A field as text: nothing is empty, a string is itself, anything else its JSON.
pub fn json_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// A named field of parameters, absent when missing or `null`.
pub fn json_field<'map>(params: &'map Map<String, Value>, name: &str) -> Option<&'map Value> {
    params.get(name).filter(|value| !value.is_null())
}
