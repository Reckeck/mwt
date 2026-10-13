//! Text nodes: finding in a string, a number out of text, splitting, joining,
//! replacing, case; and converting a value into another type.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use regex::RegexBuilder;

use crate::Amount;
use crate::calendar::Calendar;
use crate::workflow::exec::Executor;
use crate::workflow::graph::Node;
use crate::workflow::types::WfValue;

use super::{Outputs, single};

/// Longest pattern accepted, so a pasted text cannot make a huge automaton.
const MAX_PATTERN: usize = 500;

fn regex(pattern: &str, ignore_case: bool) -> Result<regex::Regex, String> {
    if pattern.len() > MAX_PATTERN {
        return Err(format!(
            "the pattern is longer than {MAX_PATTERN} characters"
        ));
    }
    RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .size_limit(1 << 20)
        .build()
        .map_err(|err| format!("the pattern does not read: {err}"))
}

fn count(value: usize) -> WfValue {
    WfValue::Number(Amount::new((value as u64).into()))
}

/// Every number in a text, read the way people write them:
/// `1 234,56`, `-12.5`, `1,234.56`, `10 000`.
pub(crate) fn numbers_in(text: &str) -> Vec<Amount> {
    // A run of digits with spaces, dots and commas inside it, maybe signed.
    let Ok(pattern) = regex(r"-?\d[\d\u{00a0}\u{202f} .,]*\d|-?\d", false) else {
        return Vec::new();
    };
    pattern
        .find_iter(text)
        .filter_map(|found| read_number(found.as_str()))
        .collect()
}

fn read_number(raw: &str) -> Option<Amount> {
    let compact: String = raw
        .chars()
        .filter(|char| !matches!(char, ' ' | '\u{00a0}' | '\u{202f}'))
        .collect();
    let last_dot = compact.rfind('.');
    let last_comma = compact.rfind(',');
    // The last separator followed by 1–2 digits (or the only one, not
    // followed by exactly three digits) is the decimal point; the rest group.
    let decimal = match (last_dot, last_comma) {
        (Some(dot), Some(comma)) => Some(dot.max(comma)),
        (Some(at), None) | (None, Some(at)) => {
            let tail = compact.len() - at - 1;
            let repeated = compact.matches(compact.as_bytes()[at] as char).count() > 1;
            (tail != 3 && !repeated).then_some(at)
        }
        (None, None) => None,
    };
    let mut normal = String::new();
    for (index, char) in compact.char_indices() {
        match char {
            '.' | ',' if Some(index) == decimal => normal.push('.'),
            '.' | ',' => {}
            other => normal.push(other),
        }
    }
    Amount::parse(&normal).ok()
}

/// A date written as text: RFC3339, `2026-11-25`, `25.11.2026`,
/// `25.11.2026 14:30`, or by a pattern of `ДД ММ ГГГГ ЧЧ мм` (Latin too).
pub(crate) fn date_from_text(text: &str, pattern: &str, cal: &Calendar) -> Option<NaiveDateTime> {
    let text = text.trim();
    if let Some(secs) = crate::pricing::history::unix_secs(text) {
        return Some(cal.local(secs));
    }
    let formats: Vec<String> = if pattern.trim().is_empty() {
        [
            "%Y-%m-%d %H:%M",
            "%Y-%m-%d",
            "%d.%m.%Y %H:%M",
            "%d.%m.%Y",
            "%d.%m.%y",
            "%d/%m/%Y",
        ]
        .iter()
        .map(|format| (*format).to_string())
        .collect()
    } else {
        let mut format = pattern.trim().to_string();
        for (token, spec) in [
            ("ГГГГ", "%Y"),
            ("YYYY", "%Y"),
            ("ММ", "%m"),
            ("MM", "%m"),
            ("ДД", "%d"),
            ("DD", "%d"),
            ("ЧЧ", "%H"),
            ("HH", "%H"),
            ("мм", "%M"),
            ("mm", "%M"),
        ] {
            format = format.replace(token, spec);
        }
        vec![format]
    };
    formats.iter().find_map(|format| {
        NaiveDateTime::parse_from_str(text, format)
            .ok()
            .or_else(|| {
                NaiveDate::parse_from_str(text, format)
                    .ok()
                    .map(|day| day.and_time(NaiveTime::default()))
            })
    })
}

impl Executor<'_> {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn eval_text(&mut self, node: &Node) -> Result<Outputs, String> {
        match node.kind.as_str() {
            "text.extract" => {
                let text = self.input(node, "text")?.as_text();
                let pattern = self.input(node, "pattern")?.as_text();
                let ignore_case = self
                    .raw_param(node, "ignore_case")
                    .as_bool()
                    .unwrap_or(true);
                let group = super::json_number(&self.raw_param(node, "group"))
                    .and_then(|group| group.into_inner().trunc().to_string().parse::<usize>().ok())
                    .unwrap_or(0);
                let found = regex(&pattern, ignore_case)?;
                let pick = |captures: regex::Captures<'_>| {
                    captures
                        .get(group)
                        .map(|part| WfValue::Text(part.as_str().to_string()))
                };
                let all: Vec<WfValue> = found.captures_iter(&text).filter_map(pick).collect();
                Ok(Outputs::from([
                    ("match", all.first().cloned().unwrap_or_default()),
                    ("found", WfValue::Bool(!all.is_empty())),
                    ("all", WfValue::List(all)),
                ]))
            }
            "text.number" => {
                let text = self.input(node, "text")?.as_text();
                let place = super::json_number(&self.raw_param(node, "index"))
                    .and_then(|place| place.into_inner().trunc().to_string().parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                let number = numbers_in(&text).into_iter().nth(place - 1);
                Ok(Outputs::from([
                    ("found", WfValue::Bool(number.is_some())),
                    ("number", number.map_or(WfValue::Null, WfValue::Number)),
                ]))
            }
            "text.split" => {
                let text = self.input(node, "text")?.as_text();
                let separator = self.input(node, "separator")?.as_text();
                let parts: Vec<WfValue> = if separator.is_empty() {
                    text.split_whitespace()
                        .map(|part| WfValue::Text(part.to_string()))
                        .collect()
                } else {
                    text.split(separator.as_str())
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                        .map(|part| WfValue::Text(part.to_string()))
                        .collect()
                };
                Ok(Outputs::from([
                    ("count", count(parts.len())),
                    ("items", WfValue::List(parts)),
                ]))
            }
            "text.replace" => {
                let text = self.input(node, "text")?.as_text();
                let find = self.input(node, "find")?.as_text();
                let with = self.input(node, "with")?.as_text();
                let result = if find.is_empty() {
                    text
                } else if self.raw_param(node, "regex").as_bool().unwrap_or(false) {
                    regex(&find, false)?
                        .replace_all(&text, with.as_str())
                        .into_owned()
                } else {
                    text.replace(&find, &with)
                };
                Ok(single("text", WfValue::Text(result)))
            }
            "text.case" => {
                let text = self.input(node, "text")?.as_text();
                let result = match self.text_param(node, "mode").as_str() {
                    "upper" => text.to_uppercase(),
                    "lower" => text.to_lowercase(),
                    "capitalize" => {
                        let trimmed = text.trim().to_lowercase();
                        let mut chars = trimmed.chars();
                        chars
                            .next()
                            .map(|first| first.to_uppercase().chain(chars).collect())
                            .unwrap_or_default()
                    }
                    _ => text.split_whitespace().collect::<Vec<_>>().join(" "),
                };
                Ok(Outputs::from([
                    ("length", count(result.chars().count())),
                    ("text", WfValue::Text(result)),
                ]))
            }
            "text.join" => {
                let items = match self.input(node, "items")? {
                    WfValue::List(items) => items,
                    WfValue::Null => Vec::new(),
                    other => vec![other],
                };
                let separator = self.input(node, "separator")?.as_text();
                let field = self.text_param(node, "field");
                let joined = items
                    .iter()
                    .map(|item| super::lists::field_of(item, field.trim()).as_text())
                    .filter(|text| !text.is_empty())
                    .collect::<Vec<_>>()
                    .join(&separator);
                Ok(single("text", WfValue::Text(joined)))
            }
            "convert.to" => {
                let value = self.input(node, "value")?;
                let target = self.text_param(node, "to");
                let cal = Calendar::new(&self.ctx.zone);
                let result = match target.as_str() {
                    "text" => (!value.is_null()).then(|| WfValue::Text(value.as_text())),
                    "bool" => match &value {
                        WfValue::Text(text) => match text.trim().to_lowercase().as_str() {
                            "да" | "yes" | "true" | "1" | "y" | "д" => Some(WfValue::Bool(true)),
                            "нет" | "no" | "false" | "0" | "n" | "н" | "" => {
                                Some(WfValue::Bool(false))
                            }
                            _ => None,
                        },
                        other => other.as_bool().map(WfValue::Bool),
                    },
                    "date" => match &value {
                        WfValue::Date(_) => Some(value.clone()),
                        other => date_from_text(
                            &other.as_text(),
                            &self.text_param(node, "date_format"),
                            &cal,
                        )
                        .map(|local| {
                            WfValue::Date(crate::calendar::iso(cal.at_local_hm(
                                local.date(),
                                chrono::Timelike::hour(&local),
                                chrono::Timelike::minute(&local),
                            )))
                        }),
                    },
                    "money" => {
                        let currency = self.text_param(node, "currency");
                        let code = value
                            .currency()
                            .filter(|code| !code.is_empty())
                            .or_else(|| Some(currency.trim().to_lowercase()))
                            .filter(|code| !code.is_empty())
                            .ok_or("choose the currency of the amount")?;
                        self.number_of_value(&value)
                            .map(|amount| WfValue::money(amount, &code))
                    }
                    "currency" => match &value {
                        WfValue::Asset(asset) => {
                            Some(WfValue::Currency(asset.symbol.to_lowercase()))
                        }
                        other => other
                            .currency()
                            .or_else(|| {
                                let text = other.as_text().trim().to_lowercase();
                                (!text.is_empty() && text.chars().all(char::is_alphanumeric))
                                    .then_some(text)
                            })
                            .map(WfValue::Currency),
                    },
                    _ => self.number_of_value(&value).map(WfValue::Number),
                };
                Ok(Outputs::from([
                    ("ok", WfValue::Bool(result.is_some())),
                    ("result", result.unwrap_or_default()),
                ]))
            }
            other => Err(format!("unknown text node {other}")),
        }
    }

    /// A number in a value: itself, an amount's, a duration in seconds, or
    /// the first number in a text.
    fn number_of_value(&self, value: &WfValue) -> Option<Amount> {
        match value {
            WfValue::Duration(secs) => Some(Amount::new((*secs).into())),
            WfValue::Bool(yes) => Some(Amount::new(u8::from(*yes).into())),
            WfValue::Text(text) => numbers_in(text).into_iter().next(),
            other => other.as_number(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    #[test]
    fn numbers_are_read_as_people_write_them() {
        assert_eq!(numbers_in("Списано 1 234,56 ₽"), vec![amount("1234.56")]);
        assert_eq!(numbers_in("total 1,234.50 USD"), vec![amount("1234.50")]);
        assert_eq!(numbers_in("10 000 руб"), vec![amount("10000")]);
        assert_eq!(numbers_in("-12.5 и 3"), vec![amount("-12.5"), amount("3")]);
        assert_eq!(numbers_in("1,234"), vec![amount("1234")]);
        assert_eq!(numbers_in("0,5"), vec![amount("0.5")]);
        assert!(numbers_in("нет чисел").is_empty());
    }

    #[test]
    fn dates_are_read_from_text() {
        let cal = Calendar::new("UTC");
        let expect = |raw: &str| NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M").unwrap();
        assert_eq!(
            date_from_text("25.11.2026", "", &cal),
            Some(expect("2026-11-25 00:00"))
        );
        assert_eq!(
            date_from_text("25.11.2026 14:30", "", &cal),
            Some(expect("2026-11-25 14:30"))
        );
        assert_eq!(
            date_from_text("11/25/2026", "ММ/ДД/ГГГГ", &cal),
            Some(expect("2026-11-25 00:00"))
        );
        assert_eq!(date_from_text("завтра", "", &cal), None);
    }

    #[test]
    fn a_broken_pattern_is_an_error_not_a_panic() {
        assert!(regex("(unclosed", false).is_err());
        assert!(regex(&"a".repeat(MAX_PATTERN + 1), false).is_err());
    }
}
