//! JSON answers: a value by a dotted path (`data.items.0.rate`, `items[0]`).

use std::str::FromStr;

use serde_json::Value;

use super::{Extracted, parse_number};
use crate::Amount;
use crate::model::datasource::{NumberFormat, SeriesPoint, SourceOutput};

/// `path` split into keys and array indexes; `items[0].rate` == `items.0.rate`.
fn segments(path: &str) -> Vec<String> {
    path.replace('[', ".")
        .replace(']', "")
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty() && *part != "$")
        .map(str::to_string)
        .collect()
}

/// The value at `path`, or `None` when a step is missing.
pub fn at_path<'doc>(root: &'doc Value, path: &str) -> Option<&'doc Value> {
    segments(path)
        .iter()
        .try_fold(root, |node, key| match node {
            Value::Array(items) => items.get(key.parse::<usize>().ok()?),
            Value::Object(map) => map.get(key),
            _ => None,
        })
}

/// A JSON number keeps its own `.`; a string is read as people write it.
fn number_of(value: &Value, format: &NumberFormat) -> Option<Amount> {
    match value {
        Value::Number(number) => rust_decimal::Decimal::from_str(&number.to_string())
            .or_else(|_| rust_decimal::Decimal::from_scientific(&number.to_string()))
            .ok()
            .map(|decimal| Amount::new(decimal) * format.multiplier),
        Value::String(text) => parse_number(text, format),
        _ => None,
    }
}

fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

pub fn extract(
    body: &[u8],
    path: &str,
    output: SourceOutput,
    format: &NumberFormat,
) -> Result<Extracted, String> {
    let root: Value =
        serde_json::from_slice(body).map_err(|err| format!("not a JSON answer: {err}"))?;
    let value = at_path(&root, path).ok_or_else(|| format!("nothing at «{path}»"))?;
    if output == SourceOutput::Series {
        let Value::Array(items) = value else {
            return Err(format!("«{path}» is not an array"));
        };
        let series = items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                Value::Array(pair) if pair.len() >= 2 => Some(SeriesPoint {
                    label: text_of(&pair[0]),
                    value: number_of(&pair[1], format)?,
                }),
                other => Some(SeriesPoint {
                    label: (index + 1).to_string(),
                    value: number_of(other, format)?,
                }),
            })
            .collect();
        return Ok(Extracted {
            series,
            ..Extracted::default()
        });
    }
    Ok(Extracted {
        number: number_of(value, format),
        text: Some(text_of(value)),
        ..Extracted::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(text: &str) -> Amount {
        Amount::new(rust_decimal::Decimal::from_str(text).unwrap())
    }

    const ANSWER: &str = r#"{"data":{"items":[{"rate":6.35,"name":"CPI"},{"rate":"6,40"}],
        "series":[["2026-08",6.4],["2026-09",6.35]],"plain":[1,2.5]}}"#;

    #[test]
    fn reads_by_path() {
        let format = NumberFormat::default();
        let got = extract(
            ANSWER.as_bytes(),
            "data.items.0.rate",
            SourceOutput::Number,
            &format,
        )
        .unwrap();
        assert_eq!(got.number, Some(amount("6.35")));
        let got = extract(
            ANSWER.as_bytes(),
            "data.items[1].rate",
            SourceOutput::Number,
            &format,
        )
        .unwrap();
        assert_eq!(got.number, Some(amount("6.40")));
        let got = extract(
            ANSWER.as_bytes(),
            "data.items.0.name",
            SourceOutput::Text,
            &format,
        )
        .unwrap();
        assert_eq!(got.text.as_deref(), Some("CPI"));
        assert!(
            extract(
                ANSWER.as_bytes(),
                "data.missing",
                SourceOutput::Number,
                &format
            )
            .is_err()
        );
        assert!(extract(b"<html>", "a", SourceOutput::Number, &format).is_err());
    }

    #[test]
    fn arrays_are_series() {
        let format = NumberFormat::default();
        let pairs = extract(
            ANSWER.as_bytes(),
            "data.series",
            SourceOutput::Series,
            &format,
        )
        .unwrap();
        assert_eq!(pairs.series[1].label, "2026-09");
        assert_eq!(pairs.series[1].value, amount("6.35"));
        let plain = extract(
            ANSWER.as_bytes(),
            "data.plain",
            SourceOutput::Series,
            &format,
        )
        .unwrap();
        assert_eq!(plain.series[1].label, "2");
        assert_eq!(plain.series[1].value, amount("2.5"));
    }
}
