//! A chain source for tests that needs no network and no HTTP client: the
//! node is a closure registered under a made-up URL, and [`Loopback`] answers
//! [`ChainSource`] calls from it. Wave-1 adapters and the EVM balance reader
//! are tested against the port, not against the `mwt-net` adapter.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use serde_json::{Value, json};

use super::ports::{ChainCall, ChainSource};
use mwt_base::error::{Error, Result};

/// The bodies a fake node was asked, in order.
pub(crate) type Seen = Arc<Mutex<Vec<Value>>>;

type Handler = Arc<dyn Fn(&Value) -> Value + Send + Sync>;

struct Node {
    handler: Handler,
    seen: Seen,
}

static NODES: LazyLock<Mutex<HashMap<String, Arc<Node>>>> = LazyLock::new(Mutex::default);
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Register a fake node: `handler` gets the JSON-RPC batch body and returns
/// the array of replies. Returns its URL and what it was asked.
pub(crate) fn serve_raw(
    handler: impl Fn(&Value) -> Value + Send + Sync + 'static,
) -> (String, Seen) {
    let url = format!("http://node-{}.test", NEXT.fetch_add(1, Ordering::Relaxed));
    let seen: Seen = Arc::default();
    let node = Arc::new(Node {
        handler: Arc::new(handler),
        seen: Arc::clone(&seen),
    });
    NODES.lock().expect("the nodes").insert(url.clone(), node);
    (url, seen)
}

/// The chain source of the tests: any URL made by [`serve_raw`] answers, any
/// other one is a dead endpoint.
#[derive(Clone, Copy, Default)]
pub(crate) struct Loopback {
    /// A reply longer than this is refused, as a client with a size cap does.
    pub(crate) max_response_bytes: Option<usize>,
}

impl Loopback {
    fn ask(&self, url: &str, body: Value) -> Result<Vec<Value>> {
        let node = NODES
            .lock()
            .expect("the nodes")
            .get(url)
            .cloned()
            .ok_or_else(|| Error::Network(format!("{url} does not answer")))?;
        node.seen.lock().expect("the log").push(body.clone());
        let reply = (node.handler)(&body);
        if let Some(limit) = self.max_response_bytes
            && reply.to_string().len() > limit
        {
            return Err(Error::Network("the answer is too large".into()));
        }
        Ok(reply.as_array().cloned().unwrap_or_default())
    }
}

fn result_of(reply: &Value) -> Result<Value> {
    match reply.get("error") {
        Some(error) => Err(Error::Network(error["message"].to_string())),
        None => Ok(reply["result"].clone()),
    }
}

impl ChainSource for Loopback {
    async fn get_json(&self, _base: &str, _path: &str, _subject: &str) -> Result<Value> {
        Err(Error::Network("the fake node speaks JSON-RPC only".into()))
    }

    async fn call(&self, url: &str, _subject: &str, method: &str, params: Value) -> Result<Value> {
        let body = json!([{"jsonrpc": "2.0", "id": 0, "method": method, "params": params}]);
        let replies = self.ask(url, body)?;
        replies
            .first()
            .ok_or_else(|| Error::Network("no reply".into()))
            .and_then(result_of)
    }

    async fn batch(
        &self,
        url: &str,
        _subject: &str,
        calls: &[ChainCall],
    ) -> Result<Vec<Result<Value>>> {
        let body: Vec<Value> = calls
            .iter()
            .enumerate()
            .map(|(id, call)| {
                json!({"jsonrpc": "2.0", "id": id, "method": call.method, "params": call.params})
            })
            .collect();
        let replies = self.ask(url, Value::Array(body))?;
        Ok(replies.iter().map(result_of).collect())
    }
}
