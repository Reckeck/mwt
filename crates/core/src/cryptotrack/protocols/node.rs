//! The node side of a protocol adapter (spec D0, addendum for wave 1).
//!
//! Every adapter of wave 1 does the same four things to a node, and they are
//! here once instead of four times:
//!
//! * pick the endpoints — the one the user set for the network, or the
//!   presets of [`crate::cryptotrack::chains`], tried in order until one
//!   answers (the same rule as
//!   [`crate::cryptotrack::sources::evm::read_balances`]);
//! * make one `eth_call`, or several for one address through Multicall3, so
//!   the per-host limiter and the "one subject per batch" rule hold;
//! * scan `Transfer` logs in ranges a public endpoint will actually answer,
//!   narrowing the range when it refuses;
//! * read the words of an answer with bounds checks ([`Answer`]) — a node's
//!   answer is untrusted input and a decoder must not index past it.
//!
//! No document is touched anywhere in this file: an adapter reads the network
//! and hands back what it found, so the vault lock is never held across a
//! request (`AGENTS.md`, rule 3).

use serde_json::{Value, json};

use crate::Amount;
use crate::cryptotrack::chains;
use crate::cryptotrack::protocols::{ProtocolRequest, U256};
use crate::cryptotrack::rpc::{RpcCall, RpcClient};
use crate::cryptotrack::sources::evm::abi;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::cryptotrack::normalize_address;

/// Blocks per `eth_getLogs` to begin with. Public endpoints cap the range
/// (commonly at 10 000 blocks) and a refusal is answered by halving, so this
/// is a starting point and not a promise.
const LOG_SPAN: u64 = 10_000;

/// The smallest range worth asking for: below this a refusal is the
/// endpoint's and not the range's, and it is reported instead of split again.
const LOG_SPAN_MIN: u64 = 500;

/// Calls per Multicall3 request. One call is a few hundred bytes of
/// calldata, and an answer that a node refuses for being too large costs a
/// whole round trip, so this stays well under
/// [`crate::cryptotrack::sources::evm::TOKENS_PER_CALL`]'s neighbourhood.
const CALLS_PER_REQUEST: usize = 40;

/// What an adapter talks to: one network, one address, the endpoints of that
/// network and the shared client.
pub struct Node<'req> {
    client: &'req RpcClient,
    chain: Chain,
    /// The address every request of this node is about: the limiter's
    /// subject, so two addresses are never mixed in one batch.
    subject: String,
    /// In the order they are tried.
    urls: Vec<&'req str>,
}

impl<'req> Node<'req> {
    /// The node of one request. `Err` when the network is not an EVM one or
    /// the address is not an address — neither is worth a request.
    pub fn new(client: &'req RpcClient, request: &ProtocolRequest<'req>) -> Result<Self> {
        let row = chains::evm_chain(request.chain).ok_or(Error::Unsupported)?;
        let subject = normalize_address(request.chain, request.address);
        let urls = match request.rpc {
            Some(url) => vec![url],
            None => row.rpc.to_vec(),
        };
        if urls.is_empty() {
            return Err(Error::Network("no RPC endpoint".into()));
        }
        Ok(Self {
            client,
            chain: request.chain,
            subject,
            urls,
        })
    }

    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// The address this node is reading for, normalized.
    pub fn address(&self) -> &str {
        &self.subject
    }

    /// One `eth_call` at the latest block: the selector, then one word per
    /// argument ([`U256::to_be_bytes`], [`abi::address_word`],
    /// [`abi::int_word`]).
    pub async fn call(&self, to: &str, selector: &str, args: &[[u8; 32]]) -> Result<Answer> {
        let data = abi::calldata(selector, args)?;
        let params = json!([{"to": to, "data": data}, "latest"]);
        let answer = self.any("eth_call", params).await?;
        let text = answer
            .as_str()
            .ok_or_else(|| Error::Network("eth_call answer is not text".into()))?;
        Answer::parse(text)
    }

    /// Several calls about this one address, as one Multicall3 request per
    /// [`CALLS_PER_REQUEST`]. `None` is a call the contract refused — a
    /// position that is gone, a reserve the user never touched — and not an
    /// error of the whole read.
    ///
    /// Each call is `(contract, calldata)`, as [`abi::calldata`] built it.
    pub async fn call_many(&self, calls: &[(String, String)]) -> Result<Vec<Option<Answer>>> {
        let mut out = Vec::with_capacity(calls.len());
        for chunk in calls.chunks(CALLS_PER_REQUEST) {
            let mut batch = Vec::with_capacity(chunk.len());
            for (target, data) in chunk {
                batch.push(abi::Call3 {
                    target: target.to_lowercase(),
                    data: unhex_calldata(data)?,
                });
            }
            let calldata = abi::aggregate3_calldata(&batch)?;
            let params = json!([{"to": abi::MULTICALL3, "data": calldata}, "latest"]);
            let answer = self.any("eth_call", params).await?;
            let text = answer
                .as_str()
                .ok_or_else(|| Error::Network("eth_call answer is not text".into()))?;
            for data in abi::decode_aggregate3(text, batch.len())? {
                out.push(match data {
                    Some(bytes) => Some(Answer::of(abi::split_words(&bytes)?)),
                    None => None,
                });
            }
        }
        Ok(out)
    }

    /// The block the node calls the latest one.
    pub async fn block_number(&self) -> Result<u64> {
        let answer = self.any("eth_blockNumber", json!([])).await?;
        let text = answer
            .as_str()
            .ok_or_else(|| Error::Network("eth_blockNumber answer is not text".into()))?;
        let digits = text
            .strip_prefix("0x")
            .ok_or_else(|| Error::Network("a block number is 0x hex".into()))?;
        u64::from_str_radix(digits, 16)
            .map_err(|_| Error::Network("a block number is 0x hex".into()))
    }

    /// Logs of one contract between two blocks, read in ranges the endpoint
    /// accepts: a refusal halves the range down to [`LOG_SPAN_MIN`], below
    /// which the refusal is the endpoint's and is reported.
    ///
    /// `topics` is the filter as JSON-RPC wants it (`null` for "any"), and
    /// the answer is every log of every range, in the order the node gave
    /// them.
    pub async fn logs(
        &self,
        address: &str,
        topics: Vec<Value>,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<Value>> {
        let mut found = Vec::new();
        let mut start = from_block;
        let mut span = LOG_SPAN;
        while start <= to_block {
            let end = start.saturating_add(span - 1).min(to_block);
            let params = json!([{
                "address": address,
                "topics": topics,
                "fromBlock": format!("0x{start:x}"),
                "toBlock": format!("0x{end:x}"),
            }]);
            match self.any("eth_getLogs", params).await {
                Ok(Value::Array(logs)) => {
                    found.extend(logs);
                    start = end + 1;
                }
                Ok(_) => return Err(Error::Network("eth_getLogs answer is not a list".into())),
                Err(err) => {
                    if span <= LOG_SPAN_MIN {
                        return Err(err);
                    }
                    span = (span / 2).max(LOG_SPAN_MIN);
                }
            }
        }
        Ok(found)
    }

    /// One call against the endpoints in turn: the first answer wins, and the
    /// last error is reported when none does.
    async fn any(&self, method: &str, params: Value) -> Result<Value> {
        let mut last = Error::Network("no RPC endpoint".into());
        for url in &self.urls {
            match self
                .client
                .call(url, &self.subject, method, params.clone())
                .await
            {
                Ok(value) => return Ok(value),
                Err(err) => last = err,
            }
        }
        Err(last)
    }

    /// Several calls in one JSON-RPC batch, for a node without Multicall3.
    /// Kept for an adapter that needs the two shapes side by side (spec D2
    /// reads `StateView` on networks where Multicall3 is not deployed).
    pub async fn batch(&self, calls: &[(String, String)]) -> Result<Vec<Result<Answer>>> {
        let requests: Vec<RpcCall> = calls
            .iter()
            .map(|(to, data)| RpcCall::new("eth_call", json!([{"to": to, "data": data}, "latest"])))
            .collect();
        let mut last = Error::Network("no RPC endpoint".into());
        for url in &self.urls {
            match self.client.batch(url, &self.subject, &requests).await {
                Ok(answers) => {
                    return Ok(answers
                        .into_iter()
                        .map(|answer| {
                            let value = answer?;
                            let text = value.as_str().ok_or_else(|| {
                                Error::Network("eth_call answer is not text".into())
                            })?;
                            Answer::parse(text)
                        })
                        .collect());
                }
                Err(err) => last = err,
            }
        }
        Err(last)
    }
}

/// The words a call answered with, read by position and bounds-checked.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Answer {
    words: Vec<[u8; 32]>,
}

impl Answer {
    pub fn of(words: Vec<[u8; 32]>) -> Self {
        Self { words }
    }

    /// From the `0x…` hex a node returns.
    pub fn parse(text: &str) -> Result<Self> {
        let digits = text
            .strip_prefix("0x")
            .ok_or_else(|| Error::Network("an answer is 0x hex".into()))?;
        if digits.len() % 2 != 0 {
            return Err(Error::Network("an answer is whole bytes".into()));
        }
        let mut bytes = Vec::with_capacity(digits.len() / 2);
        for pair in digits.as_bytes().chunks(2) {
            let text =
                std::str::from_utf8(pair).map_err(|_| Error::Network("an answer is hex".into()))?;
            bytes.push(
                u8::from_str_radix(text, 16)
                    .map_err(|_| Error::Network("an answer is hex".into()))?,
            );
        }
        Ok(Self {
            words: abi::split_words(&bytes)?,
        })
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// The whole 32-byte word at `at`.
    pub fn word(&self, at: usize) -> Result<&[u8; 32]> {
        self.words.get(at).ok_or_else(|| Self::short(at))
    }

    pub fn u256(&self, at: usize) -> Result<U256> {
        Ok(U256::from_be_bytes(self.word(at)?))
    }

    /// A `uint128` (a pool's liquidity) — `Err` when the word is wider,
    /// which means the layout read is not the one the contract answered.
    pub fn u128(&self, at: usize) -> Result<u128> {
        let word = self.word(at)?;
        if word[..16].iter().any(|byte| *byte != 0) {
            return Err(Error::Network("value wider than 128 bits".into()));
        }
        let mut tail = [0u8; 16];
        tail.copy_from_slice(&word[16..]);
        Ok(u128::from_be_bytes(tail))
    }

    /// A signed value as far as `i64` reaches: a tick (`int24`).
    pub fn int(&self, at: usize) -> Result<i64> {
        abi::word_int(self.word(at)?)
    }

    /// An address word, lowercase and `0x`-prefixed.
    pub fn address(&self, at: usize) -> Result<String> {
        abi::word_address(self.word(at)?)
    }

    /// A `bool` word: zero is false, one is true, anything else is not a
    /// bool and is refused.
    pub fn boolean(&self, at: usize) -> Result<bool> {
        let word = self.word(at)?;
        match U256::from_be_bytes(word) {
            value if value.is_zero() => Ok(false),
            value if value == U256::from_u128(1) => Ok(true),
            _ => Err(Error::Network("not a bool word".into())),
        }
    }

    /// A quantity in whole units of a token with `decimals` places.
    pub fn amount(&self, at: usize, decimals: u8) -> Result<Amount> {
        self.u256(at)?.to_amount(decimals)
    }

    fn short(at: usize) -> Error {
        Error::Network(format!("the answer has no word {at}"))
    }
}

/// `0x…` calldata back into bytes, for a call that goes inside a Multicall3.
/// Calldata is a selector and then whole words, so it is not a whole number
/// of words and is read as plain bytes.
fn unhex_calldata(text: &str) -> Result<Vec<u8>> {
    let digits = text
        .strip_prefix("0x")
        .ok_or_else(|| Error::InvalidInput("calldata is 0x hex".into()))?;
    if digits.len() % 2 != 0 {
        return Err(Error::InvalidInput("calldata is whole bytes".into()));
    }
    let mut bytes = Vec::with_capacity(digits.len() / 2);
    for pair in digits.as_bytes().chunks(2) {
        let pair =
            std::str::from_utf8(pair).map_err(|_| Error::InvalidInput("calldata is hex".into()))?;
        bytes.push(
            u8::from_str_radix(pair, 16)
                .map_err(|_| Error::InvalidInput("calldata is hex".into()))?,
        );
    }
    Ok(bytes)
}

/// A node on the loopback interface, for the tests of the adapters: the same
/// shape as the stub of [`crate::cryptotrack::rpc`]'s own tests, so a wave-1
/// spec writes fixtures and not a server.
#[cfg(test)]
pub(crate) mod stub {
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use crate::cryptotrack::rpc::{RpcClient, RpcConfig};

    /// The bodies the stub was asked, in order.
    pub(crate) type Seen = Arc<Mutex<Vec<Value>>>;

    /// A client that may talk to the loopback interface and waits for
    /// nothing: a test is not a rate limit.
    pub(crate) fn client() -> RpcClient {
        RpcClient::new(RpcConfig {
            allow_loopback_http: true,
            max_retries: 0,
            min_interval: std::time::Duration::ZERO,
            subject_gap_min: std::time::Duration::ZERO,
            subject_gap_max: std::time::Duration::ZERO,
            ..RpcConfig::default()
        })
        .expect("a loopback client")
    }

    /// Serve `answer(method, params, call_index) -> Value` (the `result` of
    /// each call) on a loopback port. Returns the URL and what was asked.
    ///
    /// `Value::Null` means the node refused that call: the reply carries a
    /// JSON-RPC error instead of a result, which is how a public endpoint
    /// says "that range is too wide" or "no such method".
    pub(crate) async fn serve(
        answer: impl Fn(&str, &Value, usize) -> Value + Send + Sync + 'static,
    ) -> (String, Seen) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let seen: Seen = Arc::default();
        let log = Arc::clone(&seen);
        let answer = Arc::new(answer);
        tokio::spawn(async move {
            let mut calls = 0usize;
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let log = Arc::clone(&log);
                let answer = Arc::clone(&answer);
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 4096];
                let body = loop {
                    let Ok(read) = stream.read(&mut chunk).await else {
                        break Value::Null;
                    };
                    if read == 0 {
                        break Value::Null;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                    let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n")
                    else {
                        continue;
                    };
                    let head = String::from_utf8_lossy(&buffer[..end]).to_ascii_lowercase();
                    let length = head
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if buffer.len() >= end + 4 + length {
                        break serde_json::from_slice::<Value>(&buffer[end + 4..end + 4 + length])
                            .unwrap_or(Value::Null);
                    }
                };
                log.lock().expect("the log").push(body.clone());
                let replies: Vec<Value> = body
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .map(|call| {
                        let method = call["method"].as_str().unwrap_or_default();
                        let result = answer(method, &call["params"], calls);
                        calls += 1;
                        if result.is_null() {
                            return json!({
                                "jsonrpc": "2.0",
                                "id": call["id"].clone(),
                                "error": {"code": -32000, "message": "refused"},
                            });
                        }
                        json!({"jsonrpc": "2.0", "id": call["id"].clone(), "result": result})
                    })
                    .collect();
                let payload = Value::Array(replies).to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            }
        });
        (url, seen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "0x1111111111111111111111111111111111111111";
    const CONTRACT: &str = "0x2222222222222222222222222222222222222222";

    fn request(url: &str) -> ProtocolRequest<'_> {
        ProtocolRequest {
            chain: Chain::Ethereum,
            address: ME,
            account_id: "cacc-1",
            since: None,
            rpc: Some(url),
            memory: None,
        }
    }

    fn word(value: u64) -> String {
        format!("{value:064x}")
    }

    #[test]
    fn an_answer_is_read_by_position_and_never_past_its_end() {
        let answer = Answer::parse(&format!("0x{}{}", word(7), word(9))).expect("two words");
        assert_eq!(answer.len(), 2);
        assert_eq!(answer.u256(0).expect("the first").to_decimal(), "7");
        assert_eq!(answer.u128(1).expect("the second"), 9);
        assert!(answer.u256(2).is_err(), "there is no third word");
        assert!(Answer::parse("0x01").is_err(), "not whole words");
        assert!(Answer::parse("00").is_err(), "no 0x");
    }

    #[test]
    fn a_signed_tick_and_an_address_are_read_the_way_the_abi_writes_them() {
        let negative = format!("0x{}", "f".repeat(64));
        assert_eq!(
            Answer::parse(&negative)
                .expect("a word")
                .int(0)
                .expect("-1"),
            -1
        );
        let address = format!("0x{}{}", "0".repeat(24), &CONTRACT[2..]);
        assert_eq!(
            Answer::parse(&address)
                .expect("a word")
                .address(0)
                .expect("an address"),
            CONTRACT
        );
        let not_an_address = format!("0x{}", "1".repeat(64));
        assert!(
            Answer::parse(&not_an_address)
                .expect("a word")
                .address(0)
                .is_err(),
            "the high bytes must be zero"
        );
        let two = format!("0x{}", word(2));
        assert!(
            Answer::parse(&two).expect("a word").boolean(0).is_err(),
            "two is not a bool"
        );
    }

    #[test]
    fn a_negative_argument_is_written_the_way_the_abi_reads_it_back() {
        // `ticks(int24)` of a Uniswap pool is asked with a negative tick
        // (spec D1): what this encoder writes is what the decoder reads.
        for tick in [-887_272i64, -1, 0, 1, 887_272] {
            let word = abi::int_word(tick);
            assert_eq!(
                Answer::of(vec![word]).int(0).expect("a tick"),
                tick,
                "tick {tick}"
            );
        }
        assert_eq!(abi::int_word(-1), [0xffu8; 32], "two's complement");
    }

    #[tokio::test]
    async fn a_call_asks_the_endpoint_the_user_set_and_reads_its_words() {
        let (url, seen) = stub::serve(|method, _params, _index| {
            assert_eq!(method, "eth_call");
            json!(format!("0x{}", word(1234)))
        })
        .await;
        let client = stub::client();
        let node = Node::new(&client, &request(&url)).expect("a node");
        let answered = node
            .call(
                CONTRACT,
                "70a08231",
                &[abi::address_word(ME).expect("a word")],
            )
            .await
            .expect("an answer");
        assert_eq!(answered.u256(0).expect("a balance").to_decimal(), "1234");
        let asked = seen.lock().expect("the log");
        assert_eq!(asked.len(), 1, "one request for one call");
        let params = &asked[0][0]["params"];
        assert_eq!(params[0]["to"], CONTRACT);
        assert!(
            params[0]["data"]
                .as_str()
                .expect("calldata")
                .starts_with("0x70a08231"),
            "the selector comes first"
        );
    }

    #[tokio::test]
    async fn a_node_tries_the_next_endpoint_when_one_does_not_answer() {
        let (good, _) =
            stub::serve(|_method, _params, _index| json!(format!("0x{}", word(5)))).await;
        let dead = "http://127.0.0.1:1";
        let client = stub::client();
        let node = Node {
            client: &client,
            chain: Chain::Ethereum,
            subject: ME.to_string(),
            urls: vec![dead, &good],
        };
        assert_eq!(
            node.call(CONTRACT, "70a08231", &[])
                .await
                .expect("the second endpoint answered")
                .u256(0)
                .expect("a word")
                .to_decimal(),
            "5"
        );
    }

    #[tokio::test]
    async fn logs_are_read_in_ranges_and_a_refused_range_is_halved() {
        // The stub refuses any range wider than 2 500 blocks, the way a
        // public endpoint does, and the reader has to find that out.
        let (url, seen) = stub::serve(|method, params, _index| {
            assert_eq!(method, "eth_getLogs");
            let filter = &params[0];
            let from = u64::from_str_radix(
                filter["fromBlock"]
                    .as_str()
                    .expect("from")
                    .trim_start_matches("0x"),
                16,
            )
            .expect("a block");
            let to = u64::from_str_radix(
                filter["toBlock"]
                    .as_str()
                    .expect("to")
                    .trim_start_matches("0x"),
                16,
            )
            .expect("a block");
            if to - from + 1 > 2_500 {
                return Value::Null;
            }
            json!([{"blockNumber": format!("0x{from:x}")}])
        })
        .await;
        let client = stub::client();
        let node = Node::new(&client, &request(&url)).expect("a node");
        let logs = node
            .logs(CONTRACT, vec![Value::Null], 1, 5_000)
            .await
            .expect("the ranges that fit");
        assert!(!logs.is_empty(), "the narrowed ranges answered");
        let asked = seen.lock().expect("the log").len();
        assert!(
            asked > 1,
            "the first, refused range is not the only request"
        );
    }
}
