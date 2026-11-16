//! The page cursor kept between calls.
//!
//! The explorer lists are read one after another (transactions, token
//! transfers, internal transfers, held tokens). A cursor names the list and
//! carries the explorer's own `next_page_params` for it. The cursor is stored
//! by the device and comes back as input, so it is validated like any other
//! untrusted text: its values end up in a URL.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

const MAX_CURSOR_LEN: usize = 2048;
const MAX_PARAMS: usize = 8;
const MAX_VALUE_LEN: usize = 100;
const VERSION: u8 = 1;

/// The explorer list being read; the order is the reading order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
    Transactions,
    TokenTransfers,
    InternalTransfers,
    Tokens,
}

impl Stream {
    /// The list after this one, `None` after the last.
    pub fn next(self) -> Option<Self> {
        match self {
            Stream::Transactions => Some(Stream::TokenTransfers),
            Stream::TokenTransfers => Some(Stream::InternalTransfers),
            Stream::InternalTransfers => Some(Stream::Tokens),
            Stream::Tokens => None,
        }
    }

    pub(super) fn path(self) -> &'static str {
        match self {
            Stream::Transactions => "transactions",
            Stream::TokenTransfers => "token-transfers",
            Stream::InternalTransfers => "internal-transactions",
            Stream::Tokens => "tokens",
        }
    }
}

/// Query parameters of the explorer's next page.
pub type PageParams = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    #[serde(rename = "v")]
    version: u8,
    #[serde(rename = "s")]
    pub stream: Stream,
    /// `None`: the first page of the list.
    #[serde(rename = "p", default, skip_serializing_if = "Option::is_none")]
    pub params: Option<PageParams>,
}

impl Cursor {
    /// The first page of the first list.
    pub fn start() -> Self {
        Self::at(Stream::Transactions, None)
    }

    pub fn at(stream: Stream, params: Option<PageParams>) -> Self {
        Self {
            version: VERSION,
            stream,
            params,
        }
    }

    /// `None` is the very beginning.
    pub fn decode(text: Option<&str>) -> Result<Self> {
        let Some(text) = text else {
            return Ok(Self::start());
        };
        if text.len() > MAX_CURSOR_LEN {
            return Err(bad("too long"));
        }
        let cursor: Cursor = serde_json::from_str(text).map_err(|_| bad("not a cursor"))?;
        if cursor.version != VERSION {
            return Err(bad("unknown version"));
        }
        if let Some(params) = &cursor.params {
            validate_params(params)?;
        }
        Ok(cursor)
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Parameters the explorer sent as `next_page_params`: a small flat object of
/// scalars with safe characters. Anything else is refused.
pub fn validate_params(params: &PageParams) -> Result<()> {
    if params.len() > MAX_PARAMS {
        return Err(bad("too many parameters"));
    }
    for (key, value) in params {
        let key_ok = !key.is_empty()
            && key.len() <= 40
            && key
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_');
        let value_ok = match value {
            Value::String(text) => {
                text.len() <= MAX_VALUE_LEN
                    && text.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
                    })
            }
            Value::Number(number) => number.is_u64() || number.is_i64(),
            Value::Bool(_) | Value::Null => true,
            Value::Array(_) | Value::Object(_) => false,
        };
        if !key_ok || !value_ok {
            return Err(bad("unsafe parameter"));
        }
    }
    Ok(())
}

/// `key=value&…` with every byte outside the unreserved set escaped.
pub fn query_string(params: &PageParams) -> String {
    params
        .iter()
        .map(|(key, value)| {
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            format!("{key}={}", escape(&text))
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn bad(why: &str) -> Error {
    Error::InvalidInput(format!("EVM cursor: {why}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn params(value: Value) -> PageParams {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn a_cursor_round_trips() {
        let cursor = Cursor::at(
            Stream::TokenTransfers,
            Some(params(json!({"block_number": 24508165, "index": 371}))),
        );
        assert_eq!(Cursor::decode(Some(&cursor.encode())).unwrap(), cursor);
        assert_eq!(Cursor::decode(None).unwrap(), Cursor::start());
    }

    #[test]
    fn a_bad_cursor_is_refused() {
        for text in [
            "",
            "{}",
            "not json",
            r#"{"v":2,"s":"transactions"}"#,
            r#"{"v":1,"s":"nope"}"#,
            r#"{"v":1,"s":"transactions","p":{"a":{"b":1}}}"#,
            r#"{"v":1,"s":"transactions","p":{"a":"x&y=z"}}"#,
            r#"{"v":1,"s":"transactions","p":{"A b":1}}"#,
            r#"{"v":1,"s":"transactions","p":{"a":1.5}}"#,
        ] {
            assert!(Cursor::decode(Some(text)).is_err(), "{text}");
        }
        assert!(Cursor::decode(Some(&"x".repeat(5000))).is_err());
    }

    #[test]
    fn the_query_escapes_what_a_url_cannot_carry() {
        let query = query_string(&params(
            json!({"inserted_at": "2026-05-04T18:05:27.232917Z", "index": 180}),
        ));
        assert_eq!(
            query,
            "index=180&inserted_at=2026-05-04T18%3A05%3A27.232917Z"
        );
    }
}
