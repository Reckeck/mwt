use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{Value, json};
use tokio::time::Instant;
use zeroize::Zeroizing;

use super::limiter::HostLimiter;
use crate::error::{Error, Result};

/// Knobs of the client. The defaults suit public endpoints.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub timeout: Duration,
    /// Retries after the first attempt.
    pub max_retries: u32,
    pub backoff_base: Duration,
    pub backoff_max: Duration,
    /// Smallest pause between two requests to one host.
    pub min_interval: Duration,
    /// Extra random pause when the next request is for another subject.
    pub subject_gap_min: Duration,
    pub subject_gap_max: Duration,
    /// Calls per HTTP request of one subject; longer lists are split.
    pub max_batch: usize,
    /// Accept `http://` to a loopback address (tests only).
    pub allow_loopback_http: bool,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            max_retries: 3,
            backoff_base: Duration::from_millis(500),
            backoff_max: Duration::from_secs(8),
            min_interval: Duration::from_millis(250),
            subject_gap_min: Duration::from_millis(400),
            subject_gap_max: Duration::from_millis(2500),
            max_batch: 50,
            allow_loopback_http: false,
        }
    }
}

/// One JSON-RPC call.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcCall {
    pub method: String,
    pub params: Value,
}

impl RpcCall {
    pub fn new(method: &str, params: Value) -> Self {
        Self {
            method: method.to_owned(),
            params,
        }
    }
}

/// The optional Blockscout key (decision R8, spec E17). It is sent only as an
/// `x-api-key` header, never in a URL, and only over `https`. Its
/// `Debug` hides the value so no log line or error can carry it.
#[derive(Clone)]
struct ApiKey(Arc<Zeroizing<String>>);

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("ApiKey(<hidden>)")
    }
}

/// Talks JSON-RPC over HTTPS. Cheap to clone; clones share the limiter.
#[derive(Debug, Clone)]
pub struct RpcClient {
    http: reqwest::Client,
    config: RpcConfig,
    limiter: Arc<HostLimiter>,
    api_key: Option<ApiKey>,
}

enum Attempt {
    Done(Value),
    /// Worth another try, with the pause the server asked for.
    Retry(Error, Option<Duration>),
    Fail(Error),
}

impl RpcClient {
    pub fn new(config: RpcConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            // A proxy from the environment would make a stub look remote and
            // a real call leave through something the user did not choose.
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| Error::Network(err.to_string()))?;
        let limiter = Arc::new(HostLimiter::new(
            config.min_interval,
            config.subject_gap_min,
            config.subject_gap_max,
        ));
        Ok(Self {
            http,
            config,
            limiter,
            api_key: None,
        })
    }

    /// A clone (sharing the limiter) that sends `key` in an `x-api-key` header on
    /// every request that goes over `https`; `None` or an empty key sends
    /// nothing. Give the keyed clone to the Blockscout reads only: a node
    /// must never see the Blockscout key.
    #[must_use]
    pub fn with_api_key(&self, key: Option<&str>) -> Self {
        let mut keyed = self.clone();
        keyed.api_key = key
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|text| ApiKey(Arc::new(Zeroizing::new(text.to_owned()))));
        keyed
    }

    /// One call for one subject (an address). Returns the `result` field.
    pub async fn call(
        &self,
        url: &str,
        subject: &str,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        let mut results = self
            .batch(url, subject, &[RpcCall::new(method, params)])
            .await?;
        results
            .pop()
            .unwrap_or_else(|| Err(Error::Network("empty answer".into())))
    }

    /// Several calls for ONE subject, sent as one JSON-RPC batch (split by
    /// [`RpcConfig::max_batch`]). The outer error is a failed transport; each
    /// inner result is one call's `result` or its JSON-RPC error. Never mix
    /// calls about different addresses in one call of this function.
    pub async fn batch(
        &self,
        url: &str,
        subject: &str,
        calls: &[RpcCall],
    ) -> Result<Vec<Result<Value>>> {
        let host = self.check_url(url)?;
        let mut out = Vec::with_capacity(calls.len());
        for chunk in calls.chunks(self.config.max_batch.max(1)) {
            let body = Value::Array(
                chunk
                    .iter()
                    .enumerate()
                    .map(|(id, call)| json!({"jsonrpc": "2.0", "id": id, "method": call.method, "params": call.params}))
                    .collect(),
            );
            let answer = self.send(url, &host, subject, Some(&body)).await?;
            out.extend(match_answers(answer, chunk.len()));
        }
        Ok(out)
    }

    /// GET `{base}{path_and_query}` for one subject and parse the body as
    /// JSON. Goes through the same URL check, per-host limiter (including the
    /// gap when the subject changes) and retry policy as [`Self::batch`].
    pub async fn get_json(&self, base: &str, path_and_query: &str, subject: &str) -> Result<Value> {
        // A path that does not start with `/` could extend the host.
        if !path_and_query.starts_with('/') {
            return Err(Error::InvalidInput("path must start with /".into()));
        }
        let url = format!("{}{path_and_query}", base.trim_end_matches('/'));
        let host = self.check_url(&url)?;
        self.send(&url, &host, subject, None).await
    }

    fn check_url(&self, url: &str) -> Result<String> {
        let parsed = reqwest::Url::parse(url)
            .map_err(|err| Error::InvalidInput(format!("RPC URL: {err}")))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| Error::InvalidInput("RPC URL has no host".into()))?;
        let loopback = matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1");
        let allowed = match parsed.scheme() {
            "https" => true,
            "http" => self.config.allow_loopback_http && loopback,
            _ => false,
        };
        if !allowed || !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(Error::InvalidInput(
                "RPC URL must be https without credentials".into(),
            ));
        }
        Ok(format!(
            "{host}:{}",
            parsed.port_or_known_default().unwrap_or(0)
        ))
    }

    /// POST when `body` is given, GET otherwise; one limiter and retry loop.
    async fn send(
        &self,
        url: &str,
        host: &str,
        subject: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let mut attempt = 0u32;
        loop {
            self.limiter.acquire(host, subject).await;
            let outcome = self.attempt(url, body).await;
            let (err, asked) = match outcome {
                Attempt::Done(value) => return Ok(value),
                Attempt::Fail(err) => return Err(err),
                Attempt::Retry(err, asked) => (err, asked),
            };
            if attempt >= self.config.max_retries {
                return Err(err);
            }
            let backoff = self
                .config
                .backoff_base
                .saturating_mul(1u32 << attempt.min(16))
                .min(self.config.backoff_max);
            let wait = asked.map_or(backoff, |secs| secs.max(backoff));
            self.limiter.delay_host(host, Instant::now() + wait);
            attempt += 1;
        }
    }

    async fn attempt(&self, url: &str, body: Option<&Value>) -> Attempt {
        let request = match body {
            Some(body) => self.http.post(url).json(body),
            None => self.http.get(url),
        };
        // The key goes in a header (not the path, so it cannot settle in a
        // URL history) and never in clear text over `http`.
        let request = match with_key_header(request, url, self.api_key.as_ref()) {
            Ok(request) => request,
            Err(err) => return Attempt::Fail(err),
        };
        let response = match request.send().await {
            Ok(response) => response,
            // Timeouts and refused connections are transient.
            Err(err) => return Attempt::Retry(Error::Network(err.without_url().to_string()), None),
        };
        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            let asked = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|text| text.trim().parse::<u64>().ok())
                .map(|secs| Duration::from_secs(secs.min(60)));
            return Attempt::Retry(Error::Network(format!("HTTP {status}")), asked);
        }
        if !status.is_success() {
            return Attempt::Fail(Error::Network(format!("HTTP {status}")));
        }
        match response.json::<Value>().await {
            Ok(value) => Attempt::Done(value),
            Err(err) => Attempt::Fail(Error::Network(format!(
                "bad JSON-RPC answer: {}",
                err.without_url()
            ))),
        }
    }
}

/// Add the key to a request, only for an `https` URL. `x-api-key` with the raw
/// key and no prefix: the header Blockscout's `BlockScoutWeb.RateLimit` reads
/// first. It falls back to an `apikey` query parameter; we never use that,
/// because a key in a URL settles in logs and history. The value is marked
/// sensitive, so reqwest leaves it out of its own debug output.
fn with_key_header(
    request: reqwest::RequestBuilder,
    url: &str,
    key: Option<&ApiKey>,
) -> Result<reqwest::RequestBuilder> {
    let Some(key) = key else {
        return Ok(request);
    };
    if !url
        .get(..8)
        .is_some_and(|head| head.eq_ignore_ascii_case("https://"))
    {
        return Ok(request);
    }
    let mut value = reqwest::header::HeaderValue::from_str(key.0.as_str())
        .map_err(|_| Error::InvalidInput("the Blockscout key cannot be sent".into()))?;
    value.set_sensitive(true);
    Ok(request.header("x-api-key", value))
}

/// Pair answers with calls by `id`; a server may reorder or drop entries.
/// A single object (some nodes answer a batch of one that way) is accepted.
fn match_answers(answer: Value, count: usize) -> Vec<Result<Value>> {
    let items = match answer {
        Value::Array(items) => items,
        other => vec![other],
    };
    let mut by_id: HashMap<u64, Value> = HashMap::new();
    for item in items {
        if let Some(id) = item.get("id").and_then(Value::as_u64) {
            by_id.insert(id, item);
        }
    }
    (0..count as u64)
        .map(|id| match by_id.remove(&id) {
            None => Err(Error::Network("no answer for a batched call".into())),
            Some(item) => match item.get("error") {
                Some(err) if !err.is_null() => Err(Error::Network(format!(
                    "RPC error {}: {}",
                    err.get("code").and_then(Value::as_i64).unwrap_or(0),
                    err.get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                ))),
                _ => Ok(item.get("result").cloned().unwrap_or(Value::Null)),
            },
        })
        .collect()
}

#[cfg(test)]
mod api_key_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Built at run time, so no key-shaped literal sits in the source.
    fn key_text() -> String {
        ["proapi", "_", "Qw93LkzP"].concat()
    }

    #[test]
    fn the_api_key_never_prints() {
        let client = RpcClient::new(RpcConfig::default())
            .unwrap()
            .with_api_key(Some(&key_text()));
        assert!(client.api_key.is_some());
        assert!(!format!("{client:?}").contains(&key_text()));
        assert!(client.with_api_key(Some("  ")).api_key.is_none());
    }

    #[test]
    fn the_key_goes_in_the_x_api_key_header_and_nowhere_else() {
        let client = reqwest::Client::new();
        let key = ApiKey(Arc::new(Zeroizing::new(key_text())));
        let url = "https://scout.example.org/api/v2/stats";
        let built = with_key_header(client.get(url), url, Some(&key))
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            built.headers().get("x-api-key").unwrap().to_str().unwrap(),
            key_text()
        );
        assert!(built.headers().get("authorization").is_none());
        assert!(!built.url().as_str().contains(&key_text()));
        assert!(built.headers().get("x-api-key").unwrap().is_sensitive());
        // No key, no header.
        let plain = with_key_header(client.get(url), url, None)
            .unwrap()
            .build()
            .unwrap();
        assert!(plain.headers().get("x-api-key").is_none());
    }

    /// Over plain `http` (the user's own node) the key is not sent at all.
    #[tokio::test]
    async fn the_key_is_never_sent_over_http() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let read = sock.read(&mut buf).await.unwrap();
            let body = "{}";
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(reply.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&buf[..read]).to_lowercase()
        });
        let client = RpcClient::new(RpcConfig {
            allow_loopback_http: true,
            max_retries: 0,
            ..RpcConfig::default()
        })
        .unwrap()
        .with_api_key(Some(&key_text()));
        client
            .get_json(&format!("http://127.0.0.1:{port}"), "/api/v2/stats", "s")
            .await
            .unwrap();
        let request = server.await.unwrap();
        assert!(request.starts_with("get /api/v2/stats"));
        assert!(!request.contains("x-api-key"));
        assert!(!request.contains("authorization"));
        assert!(!request.contains(&key_text().to_lowercase()));
    }
}
