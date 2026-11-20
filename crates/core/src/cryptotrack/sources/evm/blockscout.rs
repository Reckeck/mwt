//! History and held tokens from Blockscout API v2 (`/api/v2/addresses/{address}/…`).
//!
//! Every list is newest first and paged by the explorer's own
//! `next_page_params`; one call of [`fetch_page`] reads one page and returns
//! the cursor of the next. The answer is untrusted: each field is checked, an
//! item that fails is reported in [`SourceBatch::rejected`] and never aborts
//! the page, and a page that is not a page at all (no `items` array, an
//! impossible size, a cursor that does not move) is an error.

use std::collections::HashSet;
use std::future::Future;

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Value, json};

use super::cursor::{Cursor, PageParams, Stream, query_string, validate_params};
use super::model::{BalanceSource, RawAsset, RawBalance, RawEvent, RawKind, Rejected, SourceBatch};
use super::uint::{Uint256, shorten};
use crate::cryptotrack::chains::{self, EvmChain};
use crate::cryptotrack::ports::ChainSource;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::cryptotrack::normalize_address;

/// Blockscout pages hold 50 items; anything far above that is not a page.
const MAX_PAGE_ITEMS: usize = 200;
const MAX_TEXT: usize = 64;

/// Reads one explorer URL as JSON. The production implementation is the
/// shared rate-limited client, which keeps one *subject* (the address) per
/// request series so a provider cannot link the owner's addresses.
pub trait ExplorerApi: Send + Sync {
    /// GET `{base}{path_and_query}` and parse the body as JSON.
    fn get_json(
        &self,
        base: &str,
        path_and_query: &str,
        subject: &str,
    ) -> impl Future<Output = Result<Value>> + Send;
}

/// Every chain source reads explorer URLs with its own `get_json`, so the
/// shared client ([`ChainSource`] in `rpc/port.rs`) is the one production
/// explorer too.
impl<C: ChainSource> ExplorerApi for C {
    fn get_json(
        &self,
        base: &str,
        path_and_query: &str,
        subject: &str,
    ) -> impl Future<Output = Result<Value>> + Send {
        ChainSource::get_json(self, base, path_and_query, subject)
    }
}

/// What to read.
#[derive(Debug, Clone, Copy)]
pub struct HistoryRequest<'req> {
    pub chain: Chain,
    pub address: &'req str,
    /// The cursor of the previous batch of this source, if any.
    pub cursor: Option<&'req str>,
    /// The user's start date (RFC3339): older items are not read.
    pub since: Option<&'req str>,
    /// A Blockscout base URL instead of the preset of the network.
    pub explorer: Option<&'req str>,
}

/// Read the next page of the history.
pub async fn fetch_page<A: ExplorerApi>(
    api: &A,
    request: &HistoryRequest<'_>,
) -> Result<SourceBatch> {
    let row = chains::evm_chain(request.chain).ok_or(Error::Unsupported)?;
    let base = request
        .explorer
        .or_else(|| row.blockscout.first().copied())
        .ok_or(Error::Unsupported)?;
    let address = normalize_address(request.chain, request.address);
    if !is_address(&address) {
        return Err(Error::InvalidInput("not an EVM address".into()));
    }
    let since = request
        .since
        .map(|text| {
            DateTime::parse_from_rfc3339(text)
                .map(|time| time.with_timezone(&Utc))
                .map_err(|_| Error::InvalidInput("start date is not RFC3339".into()))
        })
        .transpose()?;
    let cursor = Cursor::decode(request.cursor)?;

    let mut path = format!("/api/v2/addresses/{address}/{}", cursor.stream.path());
    let mut query = cursor.params.as_ref().map(query_string).unwrap_or_default();
    if cursor.stream == Stream::Tokens {
        let kind = "type=ERC-20";
        query = if query.is_empty() {
            kind.into()
        } else {
            format!("{query}&{kind}")
        };
    }
    if !query.is_empty() {
        path.push('?');
        path.push_str(&query);
    }

    let page = api.get_json(base, &path, &address).await?;
    let (items, next_params) = split_page(&page)?;
    if next_params.is_some() && next_params == cursor.params {
        return Err(Error::Network("explorer cursor does not move".into()));
    }

    let ctx = Context { row, since };
    let mut batch = SourceBatch::default();
    let mut seen = HashSet::new();
    let mut reached_start = false;
    for item in items {
        match cursor.stream {
            Stream::Tokens => match parse_held_token(item, row) {
                Ok(balance) => batch.balances.push(balance),
                Err(reason) => batch.rejected.push(Rejected {
                    subject: None,
                    reason,
                }),
            },
            stream => {
                let (stop, outcome) = parse_history_item(stream, item, &ctx);
                reached_start |= stop;
                match outcome {
                    Ok(Some(event)) => {
                        if seen.insert(event.id.clone()) {
                            batch.raw_events.push(event);
                        }
                    }
                    Ok(None) => {}
                    Err(rejected) => batch.rejected.push(rejected),
                }
            }
        }
    }

    batch.next_cursor = if reached_start || next_params.is_none() {
        cursor
            .stream
            .next()
            .map(|stream| Cursor::at(stream, None).encode())
    } else {
        Some(Cursor::at(cursor.stream, next_params).encode())
    };
    Ok(batch)
}

struct Context {
    row: &'static EvmChain,
    since: Option<DateTime<Utc>>,
}

/// The `items` array and the validated `next_page_params` of an answer.
fn split_page(page: &Value) -> Result<(&Vec<Value>, Option<PageParams>)> {
    let items = page
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Network("explorer page has no items".into()))?;
    if items.len() > MAX_PAGE_ITEMS {
        return Err(Error::Network("explorer page is too large".into()));
    }
    let next = match page.get("next_page_params") {
        None | Some(Value::Null) => None,
        Some(Value::Object(map)) => {
            let params: PageParams = map.clone().into_iter().collect();
            validate_params(&params)
                .map_err(|_| Error::Network("explorer sent an unusable next page".into()))?;
            Some(params)
        }
        Some(_) => return Err(Error::Network("explorer sent a bad next page".into())),
    };
    Ok((items, next))
}

/// `(older than the start date, the item or why not)`.
fn parse_history_item(
    stream: Stream,
    item: &Value,
    ctx: &Context,
) -> (bool, std::result::Result<Option<RawEvent>, Rejected>) {
    let hash_key = if stream == Stream::Transactions {
        "hash"
    } else {
        "transaction_hash"
    };
    let hash = item
        .get(hash_key)
        .and_then(Value::as_str)
        .map(str::to_lowercase);
    let known = hash
        .as_deref()
        .filter(|text| is_hash(text))
        .map(str::to_owned);
    let reject = |reason: &str| Rejected {
        subject: known.clone(),
        reason: reason.to_owned(),
    };

    let (Some(block), Some(stamp)) = (
        item.get("block_number").and_then(Value::as_u64),
        item.get("timestamp").and_then(Value::as_str),
    ) else {
        // Pending or not yet indexed: it will show up on a later read.
        return (false, Err(reject("pending: no block or timestamp")));
    };
    let Ok(time) = DateTime::parse_from_rfc3339(stamp).map(|time| time.with_timezone(&Utc)) else {
        return (false, Err(reject("timestamp is not RFC3339")));
    };
    if ctx.since.is_some_and(|since| time < since) {
        // Newest first: everything after this item is older still.
        return (true, Ok(None));
    }
    let Some(hash) = known.clone() else {
        return (false, Err(reject("bad transaction hash")));
    };
    let time = time.to_rfc3339_opts(SecondsFormat::Secs, true);
    let outcome = match stream {
        Stream::Transactions => parse_transaction(item, ctx.row, hash, block, time),
        Stream::TokenTransfers => parse_token_transfer(item, ctx.row, hash, block, time),
        Stream::InternalTransfers => parse_internal(item, ctx.row, hash, block, time),
        Stream::Tokens => Ok(None),
    };
    (false, outcome.map_err(|reason| reject(&reason)))
}

type Parsed = std::result::Result<Option<RawEvent>, String>;

fn parse_transaction(
    item: &Value,
    row: &EvmChain,
    hash: String,
    block: u64,
    time: String,
) -> Parsed {
    let from = party(item, "from")?;
    let to = party(item, "to").or_else(|_| {
        item.get("created_contract")
            .and_then(|contract| contract.get("hash"))
            .and_then(Value::as_str)
            .and_then(clean_address)
            .ok_or_else(|| "no recipient".to_owned())
    })?;
    let value = quantity(item.get("value"))?;
    let succeeded = match item.get("status").and_then(Value::as_str) {
        Some("ok") => true,
        Some("error") => false,
        _ => return Err("transaction status unknown".into()),
    };
    let method = item
        .get("method")
        .and_then(Value::as_str)
        .and_then(clean_selector);
    let fee = item
        .get("fee")
        .filter(|fee| fee.get("type").and_then(Value::as_str) == Some("actual"))
        .and_then(|fee| quantity(fee.get("value")).ok());
    let payload = json!({
        "hash": hash, "block": block, "time": time, "from": from, "to": to,
        "value": value.as_decimal(), "method": method, "status": item.get("status"),
        "fee": fee.as_ref().map(Uint256::as_decimal),
    });
    Ok(Some(RawEvent {
        id: format!("evm:{}:{hash}:tx", row.chain_id),
        chain_id: row.chain_id,
        hash,
        kind: RawKind::Transaction,
        block,
        time,
        from,
        to,
        asset: RawAsset::Native {
            decimals: row.native.decimals,
        },
        value,
        method,
        succeeded,
        fee,
        payload: payload.to_string(),
    }))
}

fn parse_token_transfer(
    item: &Value,
    row: &EvmChain,
    hash: String,
    block: u64,
    time: String,
) -> Parsed {
    let token_type = item.get("token_type").and_then(Value::as_str).unwrap_or("");
    if token_type != "ERC-20" {
        return Err(format!("unsupported token type {}", shorten(token_type)));
    }
    let log_index = item
        .get("log_index")
        .and_then(Value::as_u64)
        .ok_or_else(|| "no log index".to_owned())?;
    let from = party(item, "from")?;
    let to = party(item, "to")?;
    let token = item.get("token").ok_or_else(|| "no token".to_owned())?;
    let asset = token_asset(token)?;
    let total = item.get("total").ok_or_else(|| "no total".to_owned())?;
    let value = quantity(total.get("value"))?;
    let payload = json!({
        "hash": hash, "log_index": log_index, "block": block, "time": time,
        "from": from, "to": to, "value": value.as_decimal(), "token": asset,
    });
    Ok(Some(RawEvent {
        id: format!("evm:{}:{hash}:{log_index}", row.chain_id),
        chain_id: row.chain_id,
        hash,
        kind: RawKind::TokenTransfer,
        block,
        time,
        from,
        to,
        asset,
        value,
        method: item
            .get("method")
            .and_then(Value::as_str)
            .and_then(clean_selector),
        succeeded: true,
        fee: None,
        payload: payload.to_string(),
    }))
}

fn parse_internal(item: &Value, row: &EvmChain, hash: String, block: u64, time: String) -> Parsed {
    let value = quantity(item.get("value"))?;
    // A call that moved no value is not a movement.
    if value.is_zero() {
        return Ok(None);
    }
    let index = item
        .get("index")
        .and_then(Value::as_u64)
        .ok_or_else(|| "no internal index".to_owned())?;
    let from = party(item, "from")?;
    let to = party(item, "to")?;
    let succeeded = item
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let payload = json!({
        "hash": hash, "index": index, "block": block, "time": time,
        "from": from, "to": to, "value": value.as_decimal(), "success": succeeded,
    });
    Ok(Some(RawEvent {
        id: format!("evm:{}:{hash}:i{index}", row.chain_id),
        chain_id: row.chain_id,
        hash,
        kind: RawKind::InternalTransfer,
        block,
        time,
        from,
        to,
        asset: RawAsset::Native {
            decimals: row.native.decimals,
        },
        value,
        method: None,
        succeeded,
        fee: None,
        payload: payload.to_string(),
    }))
}

/// One entry of `/addresses/{address}/tokens`.
fn parse_held_token(item: &Value, _row: &EvmChain) -> std::result::Result<RawBalance, String> {
    let token = item.get("token").ok_or_else(|| "no token".to_owned())?;
    let kind = token.get("type").and_then(Value::as_str).unwrap_or("");
    if kind != "ERC-20" {
        return Err(format!("unsupported token type {}", shorten(kind)));
    }
    Ok(RawBalance {
        asset: token_asset(token)?,
        value: quantity(item.get("value"))?,
        source: BalanceSource::Explorer,
    })
}

fn token_asset(token: &Value) -> std::result::Result<RawAsset, String> {
    let contract = token
        .get("address_hash")
        .and_then(Value::as_str)
        .and_then(clean_address)
        .ok_or_else(|| "bad token contract".to_owned())?;
    // Decimals arrive as a string, a number or null; above 36 is not a token
    // anyone can use and would break exact scaling.
    let decimals = match token.get("decimals") {
        Some(Value::String(text)) => text.parse::<u8>().ok(),
        Some(Value::Number(number)) => number.as_u64().and_then(|raw| u8::try_from(raw).ok()),
        _ => None,
    }
    .filter(|decimals| *decimals <= 36)
    .ok_or_else(|| "token decimals unknown".to_owned())?;
    let suspect = token
        .get("reputation")
        .and_then(Value::as_str)
        .is_some_and(|rep| rep != "ok")
        || token
            .get("is_scam")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    Ok(RawAsset::Token {
        contract,
        symbol: clean_text(token.get("symbol")),
        name: clean_text(token.get("name")),
        decimals,
        suspect,
    })
}

/// The `hash` of an address object (`from`, `to`), lowercase.
fn party(item: &Value, key: &str) -> std::result::Result<String, String> {
    item.get(key)
        .and_then(|party| party.get("hash"))
        .and_then(Value::as_str)
        .and_then(clean_address)
        .ok_or_else(|| format!("bad `{key}` address"))
}

fn quantity(value: Option<&Value>) -> std::result::Result<Uint256, String> {
    match value {
        Some(Value::String(text)) => Uint256::from_decimal(text).map_err(|err| err.to_string()),
        // An integer JSON number is exact only up to 2^64; larger ones arrive as strings.
        Some(Value::Number(number)) => number
            .as_u64()
            .map(|raw| Uint256::from_decimal(&raw.to_string()))
            .ok_or_else(|| "quantity is not an unsigned integer".to_owned())?
            .map_err(|err| err.to_string()),
        _ => Err("quantity missing".into()),
    }
}

fn clean_address(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    is_address(&lower).then_some(lower)
}

pub(super) fn is_address(text: &str) -> bool {
    text.len() == 42
        && text.starts_with("0x")
        && text[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_hash(text: &str) -> bool {
    text.len() == 66
        && text.starts_with("0x")
        && text[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn clean_selector(text: &str) -> Option<String> {
    (text.len() == 10
        && text.starts_with("0x")
        && text[2..].bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then(|| text.to_lowercase())
}

/// Token names are attacker-controlled (airdrop spam): no control characters,
/// bounded length.
fn clean_text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .map(|text| {
            text.chars()
                .filter(|ch| !ch.is_control())
                .take(MAX_TEXT)
                .collect()
        })
        .unwrap_or_default()
}
