//! Fixtures are real answers (see `fixtures/README`-style notes in each test):
//! Blockscout pages of the Ethereum Foundation multisig
//! `0xde0B295669a9FD93d5F28D9Ec85E40f4cb697BAe` (items trimmed to the first
//! few, nothing edited), internal transfers of the public Multicall3
//! contract, and node answers captured from a public RPC. No test touches the
//! network and none uses an address of the user.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::abi::{self, Call3};
use super::cursor::{Cursor, Stream};
use super::*;
use crate::cryptotrack::rpc::RpcConfig;
use crate::error::{Error, Result};
use crate::model::chain::Chain;

const ADDRESS: &str = "0xde0B295669a9FD93d5F28D9Ec85E40f4cb697BAe";
const USDT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";
const WETH: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/src/cryptotrack/sources/evm/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{path}: {err}"))
}

fn fixture_json(name: &str) -> Value {
    serde_json::from_str(&fixture(name)).unwrap()
}

type Tamper = Box<dyn Fn(&str, Value) -> Value + Send + Sync>;

/// Serves the fixtures by the page the request names; fails on demand.
struct Fake {
    requests: Mutex<Vec<String>>,
    /// Fail (once) the request with this index.
    fail_at: Mutex<Option<usize>>,
    /// Replace every answer of this stream's first page.
    tamper: Option<Tamper>,
}

impl Fake {
    fn new() -> Self {
        Self {
            requests: Mutex::default(),
            fail_at: Mutex::new(None),
            tamper: None,
        }
    }

    fn failing_at(index: usize) -> Self {
        let fake = Self::new();
        *fake.fail_at.lock().unwrap() = Some(index);
        fake
    }

    fn log(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl ExplorerApi for Fake {
    async fn get_json(&self, base: &str, path_and_query: &str, subject: &str) -> Result<Value> {
        assert_eq!(base, "https://eth.blockscout.com");
        assert_eq!(subject, ADDRESS.to_lowercase(), "one subject: the address");
        let index = {
            let mut log = self.requests.lock().unwrap();
            log.push(path_and_query.to_owned());
            log.len() - 1
        };
        if self
            .fail_at
            .lock()
            .unwrap()
            .take_if(|at| *at == index)
            .is_some()
        {
            return Err(Error::Network("connection reset".into()));
        }
        let (path, query) = path_and_query
            .split_once('?')
            .unwrap_or((path_and_query, ""));
        let stream = path.rsplit('/').next().unwrap();
        let page = match (stream, query) {
            ("transactions", "") => "blockscout.transactions.1.json",
            ("transactions", query) if query.contains("block_number=25023309") => {
                "blockscout.transactions.2.json"
            }
            ("token-transfers", "") => "blockscout.token-transfers.1.json",
            ("token-transfers", query) if query.contains("block_number=24508165") => {
                "blockscout.token-transfers.2.json"
            }
            ("internal-transactions", "") => "blockscout.internal-transactions.1.json",
            ("tokens", query) if query.contains("id=") => "blockscout.transactions.last.json",
            ("tokens", query) if query.contains("type=ERC-20") => "blockscout.tokens.1.json",
            // The last page of a list: the real answer for a page past the end.
            _ => "blockscout.transactions.last.json",
        };
        let value = fixture_json(page);
        Ok(match &self.tamper {
            Some(tamper) => tamper(path_and_query, value),
            None => value,
        })
    }
}

fn request<'req>(cursor: Option<&'req str>) -> HistoryRequest<'req> {
    HistoryRequest {
        chain: Chain::Ethereum,
        address: ADDRESS,
        cursor,
        since: None,
        explorer: None,
    }
}

/// Read everything; `retries` lets a failed page be asked again from the same cursor.
async fn read_all(
    api: &Fake,
    since: Option<&str>,
) -> (Vec<RawEvent>, Vec<RawBalance>, Vec<Rejected>, usize) {
    let (mut events, mut balances, mut rejected, mut failures) =
        (Vec::new(), Vec::new(), Vec::new(), 0);
    let mut cursor: Option<String> = None;
    for _ in 0..40 {
        let req = HistoryRequest {
            since,
            ..request(cursor.as_deref())
        };
        match fetch_page(api, &req).await {
            Ok(batch) => {
                events.extend(batch.raw_events);
                balances.extend(batch.balances);
                rejected.extend(batch.rejected);
                match batch.next_cursor {
                    Some(next) => cursor = Some(next),
                    None => return (events, balances, rejected, failures),
                }
            }
            // The cursor of the last good batch is still `cursor`: ask again.
            Err(_) => failures += 1,
        }
    }
    panic!(
        "did not finish: {:#?}",
        api.log()
            .iter()
            .skip(5)
            .map(|path| path.rsplit("addresses/").next().unwrap()[43..].to_owned())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn a_full_read_walks_all_lists_and_builds_deterministic_ids() {
    let fake = Fake::new();
    let (events, balances, rejected, _) = read_all(&fake, None).await;

    let ids: HashSet<&str> = events.iter().map(|event| event.id.as_str()).collect();
    assert_eq!(ids.len(), events.len(), "ids are unique");

    // A USDT transfer into the multisig: 10 USDT, log index 11.
    let usdt = events
        .iter()
        .find(|event| {
            event.id
                == "evm:1:0xfb5fed0996efb8919ed32d834433a9e6c0e95d838b9aa80d299b238f3b09b854:11"
        })
        .expect("token transfer");
    assert_eq!(usdt.kind, RawKind::TokenTransfer);
    assert_eq!(usdt.block, 26331158);
    assert_eq!(usdt.time, "2026-11-03T02:03:47Z");
    assert_eq!(usdt.to, ADDRESS.to_lowercase());
    assert_eq!(usdt.amount().unwrap().to_string(), "10");
    assert!(
        matches!(&usdt.asset, RawAsset::Token { contract, symbol, decimals: 6, .. } if contract == USDT && symbol == "USDT")
    );

    // A plain transaction with native value: 0.00000368 ETH, with its fee.
    let tx = events
        .iter()
        .find(|event| {
            event.id
                == "evm:1:0xffa15b703ed606d70fa99fa5e230246c09c459d8f404198b59fc31056ceeb0af:tx"
        })
        .expect("transaction");
    assert_eq!(tx.kind, RawKind::Transaction);
    assert!(tx.succeeded);
    assert_eq!(tx.amount().unwrap().to_string(), "0.00000368");
    assert!(tx.fee.is_some());

    // Internal transfers carry their own index; zero-value calls are not events.
    assert!(
        events
            .iter()
            .any(|event| event.kind == RawKind::InternalTransfer && event.id.contains(":i"))
    );
    assert!(
        events
            .iter()
            .filter(|event| event.kind == RawKind::InternalTransfer)
            .all(|event| !event.value.is_zero())
    );

    // The first transaction of the page is pending: reported, not stored.
    assert!(
        rejected
            .iter()
            .any(|item| item.reason.starts_with("pending"))
    );

    // The held-token list came last, from the explorer.
    assert_eq!(balances.len(), 6);
    assert!(
        balances
            .iter()
            .all(|balance| balance.source == BalanceSource::Explorer)
    );

    let log = fake.log();
    assert!(log[0].contains("/transactions"));
    assert!(log.iter().any(|path| path.ends_with("/tokens?type=ERC-20")));
}

#[tokio::test]
async fn an_interruption_on_a_page_resumes_from_the_cursor_without_duplicates() {
    let (whole, ..) = read_all(&Fake::new(), None).await;

    // Fail the 4th and then the 7th request: every page boundary is covered by some run.
    for failing in 0..9 {
        let fake = Fake::failing_at(failing);
        let (events, _, _, failures) = read_all(&fake, None).await;
        assert_eq!(failures, 1, "request {failing} failed once");
        let ids: Vec<&str> = events.iter().map(|event| event.id.as_str()).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(
            ids.len(),
            unique.len(),
            "no duplicates when page {failing} failed"
        );
        assert_eq!(
            ids,
            whole
                .iter()
                .map(|event| event.id.as_str())
                .collect::<Vec<_>>(),
            "same events in the same order"
        );
    }
}

#[tokio::test]
async fn a_cursor_names_the_next_page_of_the_same_list() {
    let fake = Fake::new();
    let first = fetch_page(&fake, &request(None)).await.unwrap();
    let cursor = Cursor::decode(first.next_cursor.as_deref()).unwrap();
    assert_eq!(cursor.stream, Stream::Transactions);
    let params = cursor.params.expect("explorer params kept");
    assert_eq!(params["block_number"], json!(25023309));
    assert_eq!(params["items_count"], json!(50));

    // Re-reading the same cursor gives the same ids (a page that arrives twice is harmless).
    let again = fetch_page(&fake, &request(first.next_cursor.as_deref()))
        .await
        .unwrap();
    let again2 = fetch_page(&fake, &request(first.next_cursor.as_deref()))
        .await
        .unwrap();
    assert_eq!(again.raw_events, again2.raw_events);
    let query = &fake.log()[1];
    assert!(
        query.contains("inserted_at=2026-05-04T18%3A05%3A27.232917Z"),
        "{query}"
    );
}

#[tokio::test]
async fn the_start_date_bounds_the_read() {
    // The first token-transfer page spans 2026-10-03..2026-11-03: a start date in
    // the middle drops the older items and the list is not read any further.
    let fake = Fake::new();
    let (events, ..) = read_all(&fake, Some("2026-10-08T00:00:00Z")).await;
    let transfers: Vec<&RawEvent> = events
        .iter()
        .filter(|event| event.kind == RawKind::TokenTransfer)
        .collect();
    assert_eq!(transfers.len(), 5);
    assert!(
        transfers
            .iter()
            .all(|event| event.time.as_str() >= "2026-10-08T00:00:00Z")
    );
    assert!(
        events
            .iter()
            .all(|event| event.time.as_str() >= "2026-10-08T00:00:00Z")
    );
    let tt_requests = fake
        .log()
        .iter()
        .filter(|path| path.contains("/token-transfers"))
        .count();
    assert_eq!(tt_requests, 1, "the second page was not requested");

    let bad = HistoryRequest {
        since: Some("yesterday"),
        ..request(None)
    };
    assert!(fetch_page(&Fake::new(), &bad).await.is_err());
}

#[tokio::test]
async fn uint256_keeps_full_precision() {
    let mut fake = Fake::new();
    let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
    fake.tamper = Some(Box::new(move |path, mut page| {
        if path.contains("/token-transfers") && !path.contains('?') {
            page["items"][0]["total"]["value"] = json!(max);
            page["items"][1]["total"]["value"] = json!("123456789012345678901234567890123456789");
        }
        if path.contains("/transactions") && !path.contains("?") {
            // item 0 is pending; item 3 carries 22 digits of wei.
            page["items"][3]["value"] = json!("1000000000000000000001");
        }
        page
    }));
    let (events, ..) = read_all(&fake, None).await;
    let by_value = |digits: &str| {
        events
            .iter()
            .find(|event| event.value.as_decimal() == digits)
            .unwrap()
    };

    let huge = by_value(max);
    assert_eq!(huge.value.as_decimal(), max, "all 78 digits kept");
    assert!(huge.payload.contains(max), "and in the stored payload");
    assert!(
        huge.amount().is_err(),
        "does not fit an Amount: an error, never a rounded number"
    );

    let wide = by_value("123456789012345678901234567890123456789");
    assert!(wide.amount().is_err());

    let wei = by_value("1000000000000000000001");
    assert_eq!(wei.amount().unwrap().to_string(), "1000.000000000000000001");
}

#[tokio::test]
async fn a_hostile_page_is_an_error_and_a_hostile_item_is_rejected() {
    // No items array.
    let mut fake = Fake::new();
    fake.tamper = Some(Box::new(|_, _| json!({"message": "Too many requests"})));
    assert!(fetch_page(&fake, &request(None)).await.is_err());

    // Items is not an array, next page is not an object.
    fake.tamper = Some(Box::new(
        |_, _| json!({"items": "x", "next_page_params": null}),
    ));
    assert!(fetch_page(&fake, &request(None)).await.is_err());
    fake.tamper = Some(Box::new(
        |_, _| json!({"items": [], "next_page_params": "next"}),
    ));
    assert!(fetch_page(&fake, &request(None)).await.is_err());

    // Unusable next page params (nested, injected).
    fake.tamper = Some(Box::new(
        |_, _| json!({"items": [], "next_page_params": {"a": {"b": 1}}}),
    ));
    assert!(fetch_page(&fake, &request(None)).await.is_err());
    fake.tamper = Some(Box::new(
        |_, _| json!({"items": [], "next_page_params": {"a": "1&admin=1"}}),
    ));
    assert!(fetch_page(&fake, &request(None)).await.is_err());

    // More items than any real page holds.
    fake.tamper = Some(Box::new(
        |_, _| json!({"items": vec![json!({}); 201], "next_page_params": null}),
    ));
    assert!(fetch_page(&fake, &request(None)).await.is_err());

    // A cursor that does not move would loop forever.
    let first = fetch_page(&Fake::new(), &request(None)).await.unwrap();
    let mut stuck = Fake::new();
    stuck.tamper = Some(Box::new(|_, _| {
        fixture_json("blockscout.transactions.1.json")
    }));
    let err = fetch_page(&stuck, &request(first.next_cursor.as_deref()))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("does not move"));

    // Bad items: reported one by one, the rest of the page survives, unknown fields are fine.
    let mut mixed = Fake::new();
    mixed.tamper = Some(Box::new(|path, mut page| {
        if path.contains("/transactions") && !path.contains('?') {
            page["items"][1]["from"]["hash"] = json!("0xnot-an-address");
            page["items"][2]["hash"] = json!("0x12");
            page["items"][3]["value"] = json!("-5");
            page["items"][4]["value"] = json!(1.5);
            page["items"][5]["status"] = json!("weird");
            page["items"][6]["brand_new_field"] = json!({"x": [1, 2, 3]});
        }
        page
    }));
    let batch = fetch_page(&mixed, &request(None)).await.unwrap();
    assert_eq!(batch.raw_events.len(), 2, "items 6 and 7 are good");
    assert_eq!(batch.rejected.len(), 6, "pending + five bad items");
}

#[tokio::test]
async fn non_fungible_and_odd_tokens_are_not_taken_as_amounts() {
    let mut fake = Fake::new();
    fake.tamper = Some(Box::new(|path, mut page| {
        if path.contains("/token-transfers") && !path.contains('?') {
            page["items"][0]["token_type"] = json!("ERC-721");
            page["items"][1]["token"]["decimals"] = json!(null);
            page["items"][2]["token"]["decimals"] = json!("200");
            page["items"][3]["token"]["symbol"] = json!("A\u{0007}\u{202e}B");
            page["items"][4]["token"]["reputation"] = json!("scam");
        }
        page
    }));
    let mut cursor = Some(Cursor::at(Stream::TokenTransfers, None).encode());
    let batch = fetch_page(&fake, &request(cursor.take().as_deref()))
        .await
        .unwrap();
    assert_eq!(batch.rejected.len(), 3);
    let symbols: Vec<(&str, bool)> = batch
        .raw_events
        .iter()
        .filter_map(|event| match &event.asset {
            RawAsset::Token {
                symbol, suspect, ..
            } => Some((symbol.as_str(), *suspect)),
            RawAsset::Native { .. } => None,
        })
        .collect();
    assert!(
        symbols.iter().any(|(symbol, _)| *symbol == "A\u{202e}B"),
        "control characters stripped: {symbols:?}"
    );
    assert!(
        symbols.iter().any(|(_, suspect)| *suspect),
        "low reputation is flagged"
    );
}

#[tokio::test]
async fn unsupported_networks_and_addresses_are_refused() {
    let fake = Fake::new();
    let bsc = HistoryRequest {
        chain: Chain::Bsc,
        ..request(None)
    };
    assert!(
        matches!(fetch_page(&fake, &bsc).await, Err(Error::Unsupported)),
        "no Blockscout preset for BNB"
    );
    let btc = HistoryRequest {
        chain: Chain::Bitcoin,
        ..request(None)
    };
    assert!(matches!(
        fetch_page(&fake, &btc).await,
        Err(Error::Unsupported)
    ));
    let junk = HistoryRequest {
        address: "0x12",
        ..request(None)
    };
    assert!(fetch_page(&fake, &junk).await.is_err());
    assert!(fake.log().is_empty(), "nothing was sent");
}

// ---- balances ---------------------------------------------------------------

/// Serve JSON-RPC bodies on a loopback port; the closure sees the parsed body.
async fn rpc_stub(
    handler: impl Fn(&Value) -> String + Send + Sync + 'static,
) -> (String, std::sync::Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
    let log = std::sync::Arc::clone(&seen);
    let handler = std::sync::Arc::new(handler);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let (log, handler) = (std::sync::Arc::clone(&log), std::sync::Arc::clone(&handler));
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let body = loop {
                    let Ok(read) = stream.read(&mut chunk).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..read]);
                    let Some(end) = buf.windows(4).position(|win| win == b"\r\n\r\n") else {
                        continue;
                    };
                    let head = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|val| val.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if buf.len() >= end + 4 + len {
                        break serde_json::from_slice::<Value>(&buf[end + 4..end + 4 + len])
                            .unwrap_or(Value::Null);
                    }
                };
                log.lock().unwrap().push(body.clone());
                let text = handler(&body);
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                    text.len()
                );
                let _ = stream.write_all(reply.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (url, seen)
}

fn client() -> RpcClient {
    RpcClient::new(RpcConfig {
        timeout: Duration::from_millis(500),
        max_retries: 0,
        backoff_base: Duration::from_millis(5),
        backoff_max: Duration::from_millis(5),
        min_interval: Duration::ZERO,
        subject_gap_min: Duration::ZERO,
        subject_gap_max: Duration::ZERO,
        max_batch: 50,
        allow_loopback_http: true,
        max_response_bytes: 16 * 1024 * 1024,
    })
    .unwrap()
}

fn token(contract: &str, symbol: &str, decimals: u8) -> RawAsset {
    RawAsset::Token {
        contract: contract.into(),
        symbol: symbol.into(),
        name: symbol.into(),
        decimals,
        suspect: false,
    }
}

/// The recorded answer of a node, re-addressed to the call that asked.
fn answer(body: &Value, result: &Value) -> Value {
    json!([{"jsonrpc": "2.0", "id": body[0]["id"], "result": result}])
}

#[tokio::test]
async fn balances_come_from_one_multicall_with_exact_quantities() {
    let aggregate = fixture_json("rpc.aggregate3.json")["result"].clone();
    let (url, seen) = rpc_stub(move |body| answer(body, &aggregate).to_string()).await;
    let tokens = [token(USDT, "USDT", 6), token(WETH, "WETH", 18)];
    let batch = read_balances(
        &client(),
        &BalanceRequest {
            chain: Chain::Ethereum,
            address: ADDRESS,
            tokens: &tokens,
            rpc: Some(&url),
        },
    )
    .await
    .unwrap();

    let values: Vec<(String, &str)> = batch
        .balances
        .iter()
        .map(|bal| (bal.amount().unwrap().to_string(), bal.value.as_decimal()))
        .collect();
    assert_eq!(
        values[0],
        (
            "4774.551794456062094343".to_owned(),
            "4774551794456062094343"
        )
    );
    assert_eq!(values[1], ("1083.644119".to_owned(), "1083644119"));
    assert_eq!(
        values[2],
        ("214.140968121599991968".to_owned(), "214140968121599991968")
    );
    assert!(
        batch
            .balances
            .iter()
            .all(|balance| balance.source == BalanceSource::Node)
    );
    assert!(batch.next_cursor.is_none() && batch.rejected.is_empty());

    // One request, for one address, and the calldata is the one the real node answered.
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0][0]["method"], "eth_call");
    assert_eq!(seen[0][0]["params"][0]["to"], abi::MULTICALL3);
    assert_eq!(
        seen[0][0]["params"][0]["data"],
        fixture("rpc.aggregate3.calldata.txt").trim()
    );
}

#[tokio::test]
async fn a_node_without_multicall_is_read_with_one_plain_batch() {
    let balance = fixture_json("rpc.getBalance.json")["result"].clone();
    let of = fixture_json("rpc.balanceOf.json")["result"].clone();
    let (url, seen) = rpc_stub(move |body| {
        let calls = body.as_array().unwrap();
        if calls.iter().any(|call| call["method"] == "eth_getBalance") {
            let items: Vec<Value> = calls
                .iter()
                .map(|call| {
                    let result = if call["method"] == "eth_getBalance" {
                        &balance
                    } else {
                        &of
                    };
                    json!({"id": call["id"], "result": result})
                })
                .collect();
            Value::Array(items).to_string()
        } else {
            json!([{"id": 0, "error": {"code": -32000, "message": "execution reverted"}}])
                .to_string()
        }
    })
    .await;
    let tokens = [token(USDT, "USDT", 6)];
    let batch = read_balances(
        &client(),
        &BalanceRequest {
            chain: Chain::Ethereum,
            address: ADDRESS,
            tokens: &tokens,
            rpc: Some(&url),
        },
    )
    .await
    .unwrap();
    assert_eq!(batch.balances.len(), 2);
    assert_eq!(
        batch.balances[0].amount().unwrap().to_string(),
        "4774.551794456062094343"
    );
    assert_eq!(
        batch.balances[1].amount().unwrap().to_string(),
        "1083.644119"
    );
    assert_eq!(
        seen.lock().unwrap().len(),
        2,
        "the refused multicall, then one batch"
    );
}

#[tokio::test]
async fn a_garbage_answer_is_not_trusted() {
    let aggregate = fixture_json("rpc.aggregate3.json")["result"]
        .as_str()
        .unwrap()
        .to_owned();
    // Truncated answer: the decoder must refuse, then the plain batch is tried and also fails.
    let truncated = aggregate[..aggregate.len() - 40].to_owned();
    let (url, _) = rpc_stub(move |body| {
        if body[0]["method"] == "eth_call" {
            answer(body, &json!(truncated)).to_string()
        } else {
            json!([{"id": 0, "result": "0xzz"}]).to_string()
        }
    })
    .await;
    let tokens = [token(USDT, "USDT", 6), token(WETH, "WETH", 18)];
    let outcome = read_balances(
        &client(),
        &BalanceRequest {
            chain: Chain::Ethereum,
            address: ADDRESS,
            tokens: &tokens,
            rpc: Some(&url),
        },
    )
    .await;
    assert!(outcome.is_err());
}

#[test]
fn the_abi_encoder_matches_the_real_calldata_and_the_decoder_never_panics() {
    let owner = ADDRESS.to_lowercase();
    let calls = [
        Call3 {
            target: abi::MULTICALL3.to_lowercase(),
            data: abi::get_eth_balance_data(&owner).unwrap(),
        },
        Call3 {
            target: USDT.into(),
            data: abi::balance_of_data(&owner).unwrap(),
        },
        Call3 {
            target: WETH.into(),
            data: abi::balance_of_data(&owner).unwrap(),
        },
    ];
    assert_eq!(
        abi::aggregate3_calldata(&calls).unwrap(),
        fixture("rpc.aggregate3.calldata.txt").trim()
    );

    let good = fixture_json("rpc.aggregate3.json")["result"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(abi::decode_aggregate3(&good, 3).unwrap().len(), 3);
    assert!(abi::decode_aggregate3(&good, 2).is_err(), "wrong count");
    // Every truncation and a few corruptions: an error or a value, never a panic.
    for cut in (0..good.len()).step_by(2) {
        let _ = abi::decode_aggregate3(&good[..cut], 3);
    }
    let huge_offset = format!("0x{}{}", "ff".repeat(32), "00".repeat(64));
    assert!(abi::decode_aggregate3(&huge_offset, 1).is_err());
    assert!(abi::decode_aggregate3("0x12", 1).is_err());
    assert!(abi::decode_aggregate3("12", 1).is_err());
    assert!(abi::balance_of_calldata("0x12").is_err());
}
