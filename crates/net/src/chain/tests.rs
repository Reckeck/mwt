//! The client against a local stub server: no network leaves the machine.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::{RpcCall, RpcClient, RpcConfig};

/// What the stub saw: the time and the parsed JSON body of each request.
type Seen = Arc<Mutex<Vec<(Instant, Value)>>>;

/// A reply: status, extra header, body.
type Reply = (u16, Option<&'static str>, String);

/// Serve `handler` on a loopback port; returns the URL and the request log.
async fn stub(handler: impl Fn(usize, &Value) -> Reply + Send + Sync + 'static) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = Arc::clone(&seen);
    let handler = Arc::new(handler);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let log = Arc::clone(&log);
            let handler = Arc::clone(&handler);
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
                        if len == 0 {
                            // A GET: the "body" the stub records is the request path.
                            let line = String::from_utf8_lossy(&buf[..end]);
                            let path = line.split_whitespace().nth(1).unwrap_or("");
                            break Value::String(path.to_owned());
                        }
                        break serde_json::from_slice::<Value>(&buf[end + 4..end + 4 + len])
                            .unwrap_or(Value::Null);
                    }
                };
                let index = {
                    let mut seen = log.lock().unwrap();
                    seen.push((Instant::now(), body.clone()));
                    seen.len() - 1
                };
                let (status, header, text) = handler(index, &body);
                let extra = header.map(|line| format!("{line}\r\n")).unwrap_or_default();
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{text}",
                    text.len()
                );
                let _ = stream.write_all(reply.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (url, seen)
}

fn fast() -> RpcConfig {
    RpcConfig {
        timeout: Duration::from_millis(400),
        max_retries: 3,
        backoff_base: Duration::from_millis(10),
        backoff_max: Duration::from_millis(40),
        min_interval: Duration::ZERO,
        subject_gap_min: Duration::ZERO,
        subject_gap_max: Duration::ZERO,
        max_batch: 50,
        max_response_bytes: 16 * 1024 * 1024,
        allow_loopback_http: true,
    }
}

/// Echo every call of a batch back with `result = method`.
fn echo(body: &Value) -> String {
    let items = body.as_array().cloned().unwrap_or_default();
    Value::Array(
        items
            .iter()
            .map(|item| json!({"jsonrpc": "2.0", "id": item["id"], "result": item["method"]}))
            .collect(),
    )
    .to_string()
}

#[tokio::test]
async fn a_batch_is_one_request_and_answers_are_matched_by_id() {
    let (url, seen) = stub(|_, body| {
        // Answer in reverse order: ids, not positions, pair the answers.
        let mut items = body.as_array().cloned().unwrap_or_default();
        items.reverse();
        let answers: Vec<Value> = items
            .iter()
            .map(|item| json!({"id": item["id"], "result": item["method"]}))
            .collect();
        (200, None, Value::Array(answers).to_string())
    })
    .await;
    let client = RpcClient::new(fast()).unwrap();
    let calls = [
        RpcCall::new("eth_blockNumber", json!([])),
        RpcCall::new("eth_getBalance", json!(["0xaa", "latest"])),
    ];
    let out = client.batch(&url, "0xaa", &calls).await.unwrap();
    assert_eq!(out[0].as_ref().unwrap(), "eth_blockNumber");
    assert_eq!(out[1].as_ref().unwrap(), "eth_getBalance");
    assert_eq!(
        seen.lock().unwrap().len(),
        1,
        "one HTTP request for the batch"
    );
}

#[tokio::test]
async fn a_json_rpc_error_is_a_per_call_error() {
    let (url, _) = stub(|_, _| {
        (200, None, json!([{"id": 0, "error": {"code": -32000, "message": "nope"}}, {"id": 1, "result": "0x1"}]).to_string())
    })
    .await;
    let client = RpcClient::new(fast()).unwrap();
    let calls = [RpcCall::new("a", json!([])), RpcCall::new("b", json!([]))];
    let out = client.batch(&url, "s", &calls).await.unwrap();
    assert!(out[0].as_ref().unwrap_err().to_string().contains("-32000"));
    assert_eq!(out[1].as_ref().unwrap(), "0x1");
}

#[tokio::test]
async fn a_long_list_is_split_and_never_exceeds_max_batch() {
    let (url, seen) = stub(|_, body| (200, None, echo(body))).await;
    let client = RpcClient::new(RpcConfig {
        max_batch: 2,
        ..fast()
    })
    .unwrap();
    let calls: Vec<RpcCall> = (0..5).map(|_| RpcCall::new("m", json!([]))).collect();
    let out = client.batch(&url, "s", &calls).await.unwrap();
    assert_eq!(out.len(), 5);
    let sizes: Vec<usize> = seen
        .lock()
        .unwrap()
        .iter()
        .map(|(_, body)| body.as_array().unwrap().len())
        .collect();
    assert_eq!(sizes, vec![2, 2, 1]);
}

#[tokio::test]
async fn throttling_and_server_errors_are_retried_with_backoff() {
    let (url, seen) = stub(|index, body| match index {
        0 => (429, Some("retry-after: 0"), String::new()),
        1 => (503, None, String::new()),
        _ => (200, None, echo(body)),
    })
    .await;
    let client = RpcClient::new(fast()).unwrap();
    let value = client
        .call(&url, "s", "eth_chainId", json!([]))
        .await
        .unwrap();
    assert_eq!(value, "eth_chainId");
    let times: Vec<Instant> = seen.lock().unwrap().iter().map(|(time, _)| *time).collect();
    assert_eq!(times.len(), 3);
    assert!(
        times[1] - times[0] >= Duration::from_millis(10),
        "first backoff"
    );
    assert!(
        times[2] - times[1] >= Duration::from_millis(20),
        "backoff grows"
    );
}

#[tokio::test]
async fn retries_stop_after_max_retries() {
    let (url, seen) = stub(|_, _| (500, None, String::new())).await;
    let client = RpcClient::new(RpcConfig {
        max_retries: 2,
        ..fast()
    })
    .unwrap();
    let err = client.call(&url, "s", "m", json!([])).await.unwrap_err();
    assert!(err.to_string().contains("500"));
    assert_eq!(seen.lock().unwrap().len(), 3, "first try + 2 retries");
}

#[tokio::test]
async fn a_client_error_is_not_retried() {
    let (url, seen) = stub(|_, _| (404, None, String::new())).await;
    let client = RpcClient::new(fast()).unwrap();
    assert!(client.call(&url, "s", "m", json!([])).await.is_err());
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_slow_server_times_out_and_is_retried() {
    // Accept and never answer.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&hits);
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            *counter.lock().unwrap() += 1;
            held.push(stream);
        }
    });
    let config = RpcConfig {
        timeout: Duration::from_millis(80),
        max_retries: 1,
        ..fast()
    };
    let client = RpcClient::new(config).unwrap();
    assert!(client.call(&url, "s", "m", json!([])).await.is_err());
    assert_eq!(*hits.lock().unwrap(), 2);
}

#[tokio::test]
async fn different_subjects_get_separate_requests_spread_in_time() {
    let (url, seen) = stub(|_, body| (200, None, echo(body))).await;
    let config = RpcConfig {
        min_interval: Duration::from_millis(20),
        subject_gap_min: Duration::from_millis(120),
        subject_gap_max: Duration::from_millis(160),
        ..fast()
    };
    let client = RpcClient::new(config).unwrap();
    let first = client.clone();
    let second = client.clone();
    let (url_a, url_b) = (url.clone(), url);
    // Started together, as a scheduler would start two accounts at once.
    let (one, two) = tokio::join!(
        async move {
            first
                .call(&url_a, "0xaaaa", "eth_getBalance", json!(["0xaaaa"]))
                .await
        },
        async move {
            second
                .call(&url_b, "0xbbbb", "eth_getBalance", json!(["0xbbbb"]))
                .await
        },
    );
    one.unwrap();
    two.unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "never one shared request");
    for (_, body) in seen.iter() {
        let text = body.to_string();
        assert!(
            !(text.contains("0xaaaa") && text.contains("0xbbbb")),
            "addresses stay apart: {text}"
        );
    }
    assert!(
        seen[1].0 - seen[0].0 >= Duration::from_millis(120),
        "spread in time"
    );
}

#[tokio::test]
async fn the_same_subject_only_waits_the_minimum_interval() {
    let (url, seen) = stub(|_, body| (200, None, echo(body))).await;
    let config = RpcConfig {
        min_interval: Duration::from_millis(60),
        subject_gap_min: Duration::from_secs(5),
        subject_gap_max: Duration::from_secs(5),
        ..fast()
    };
    let client = RpcClient::new(config).unwrap();
    let start = Instant::now();
    client.call(&url, "0xaaaa", "m", json!([])).await.unwrap();
    client.call(&url, "0xaaaa", "m", json!([])).await.unwrap();
    assert!(start.elapsed() >= Duration::from_millis(60));
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "no subject gap within one subject"
    );
    assert_eq!(seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn plain_http_and_other_schemes_are_refused() {
    let strict = RpcClient::new(RpcConfig {
        allow_loopback_http: false,
        ..fast()
    })
    .unwrap();
    for url in [
        "http://127.0.0.1:1",
        "http://rpc.example.org",
        "javascript:alert(1)",
        "ftp://rpc.example.org",
        "https://user:pw@rpc.example.org",
    ] {
        assert!(
            strict.call(url, "s", "m", json!([])).await.is_err(),
            "{url}"
        );
    }
    // Even with the test switch, only loopback may be plain http.
    let loose = RpcClient::new(fast()).unwrap();
    assert!(
        loose
            .call("http://rpc.example.org", "s", "m", json!([]))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn get_json_returns_the_parsed_body() {
    let (url, seen) = stub(|_, _| (200, None, json!({"items": [1, 2]}).to_string())).await;
    let client = RpcClient::new(fast()).unwrap();
    let out = client
        .get_json(&format!("{url}/"), "/api/v2/x?y=1", "0xaaaa")
        .await
        .unwrap();
    assert_eq!(out, json!({"items": [1, 2]}));
    assert_eq!(seen.lock().unwrap()[0].1, json!("/api/v2/x?y=1"));
    // A path that could change the host is refused before any request.
    assert!(client.get_json(&url, "@evil.example/", "s").await.is_err());
}

#[tokio::test]
async fn get_json_refuses_plain_http_and_credentials() {
    let strict = RpcClient::new(RpcConfig {
        allow_loopback_http: false,
        ..fast()
    })
    .unwrap();
    for base in [
        "http://127.0.0.1:1",
        "http://explorer.example.org",
        "https://user:pw@explorer.example.org",
    ] {
        assert!(strict.get_json(base, "/a", "s").await.is_err(), "{base}");
    }
}

#[tokio::test]
async fn get_json_retries_429_with_retry_after_but_not_404() {
    let (url, seen) = stub(|index, _| {
        if index == 0 {
            (429, Some("retry-after: 1"), String::new())
        } else {
            (200, None, json!({"ok": true}).to_string())
        }
    })
    .await;
    let client = RpcClient::new(fast()).unwrap();
    let start = Instant::now();
    let out = client.get_json(&url, "/a", "s").await.unwrap();
    assert_eq!(out, json!({"ok": true}));
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert!(
        start.elapsed() >= Duration::from_millis(900),
        "Retry-After honoured"
    );

    let (url, seen) = stub(|_, _| (404, None, String::new())).await;
    assert!(client.get_json(&url, "/a", "s").await.is_err());
    assert_eq!(seen.lock().unwrap().len(), 1, "a 404 is not retried");
}

#[tokio::test]
async fn get_json_spaces_subjects_but_not_repeats_of_one_subject() {
    let (url, seen) = stub(|_, _| (200, None, "{}".to_owned())).await;
    let config = RpcConfig {
        min_interval: Duration::from_millis(20),
        subject_gap_min: Duration::from_millis(120),
        subject_gap_max: Duration::from_millis(160),
        ..fast()
    };
    let client = RpcClient::new(config).unwrap();
    let (one, two) = tokio::join!(
        client.get_json(&url, "/api/v2/addresses/0xaaaa/transactions", "0xaaaa"),
        client.get_json(&url, "/api/v2/addresses/0xbbbb/transactions", "0xbbbb"),
    );
    one.unwrap();
    two.unwrap();
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "one request per subject");
        for (_, path) in seen.iter() {
            let text = path.to_string();
            assert!(
                !(text.contains("0xaaaa") && text.contains("0xbbbb")),
                "addresses stay apart: {text}"
            );
        }
        assert!(
            seen[1].0 - seen[0].0 >= Duration::from_millis(120),
            "different subjects are spread in time"
        );
    }

    // One subject twice: only the minimum interval, never the subject gap.
    let (url, _) = stub(|_, _| (200, None, "{}".to_owned())).await;
    let config = RpcConfig {
        min_interval: Duration::from_millis(60),
        subject_gap_min: Duration::from_secs(5),
        subject_gap_max: Duration::from_secs(5),
        ..fast()
    };
    let client = RpcClient::new(config).unwrap();
    let start = Instant::now();
    client.get_json(&url, "/a", "0xaaaa").await.unwrap();
    client.get_json(&url, "/b", "0xaaaa").await.unwrap();
    assert!(start.elapsed() >= Duration::from_millis(60));
    assert!(start.elapsed() < Duration::from_secs(2));
}

fn capped(max_response_bytes: usize) -> RpcClient {
    RpcClient::new(RpcConfig {
        max_response_bytes,
        ..fast()
    })
    .unwrap()
}

/// Valid JSON of exactly `len` bytes: a string inside an object.
fn json_of_len(len: usize) -> String {
    let filler = "x".repeat(len - r#"{"a":""}"#.len());
    format!(r#"{{"a":"{filler}"}}"#)
}

#[test]
fn the_default_cap_is_16_mib() {
    assert_eq!(RpcConfig::default().max_response_bytes, 16 * 1024 * 1024);
}

#[tokio::test]
async fn an_answer_over_the_cap_is_an_error_and_is_not_retried() {
    let (url, seen) = stub(|_, _| (200, None, json_of_len(4096))).await;
    let err = capped(1024)
        .get_json(&url, "/x", "s")
        .await
        .expect_err("over the cap");
    assert!(
        matches!(&err, mwt_base::error::Error::Network(text) if text.contains("larger than the limit")),
        "{err:?}"
    );
    assert_eq!(seen.lock().unwrap().len(), 1, "no retry");
}

#[tokio::test]
async fn a_call_answer_over_the_cap_gives_no_partial_value() {
    let (url, seen) = stub(|_, _| (200, None, json_of_len(4096))).await;
    let err = capped(1024)
        .call(&url, "s", "eth_getLogs", json!([]))
        .await
        .expect_err("over the cap");
    assert!(err.to_string().contains("larger than the limit"), "{err}");
    assert_eq!(seen.lock().unwrap().len(), 1, "no retry");
}

#[tokio::test]
async fn a_declared_length_over_the_cap_fails_before_the_body_is_read() {
    // The head promises 1 MiB; the body never comes. Reading it would wait
    // for the timeout and fail with another message.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await;
        let head =
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 1048576\r\n\r\n{";
        stream.write_all(head.as_bytes()).await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
    });
    let started = Instant::now();
    let err = capped(1024)
        .get_json(&url, "/x", "s")
        .await
        .expect_err("declared too large");
    assert!(err.to_string().contains("larger than the limit"), "{err}");
    assert!(started.elapsed() < Duration::from_millis(300));
}

#[tokio::test]
async fn an_answer_just_under_the_cap_parses() {
    let (url, _) = stub(|_, _| (200, None, json_of_len(1024))).await;
    let value = capped(1024).get_json(&url, "/x", "s").await.unwrap();
    assert_eq!(value["a"].as_str().unwrap().len(), 1024 - 8);
}
