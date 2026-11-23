//! The network adapter of the `ChainSource` port, against a loopback stub: no traffic leaves the
//! machine.

use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::RpcChain;
use crate::chain::{RpcClient, RpcConfig};
use crate::cryptotrack::ports::{ChainCall, ChainSource};

/// Serve one request on a loopback port and answer every call of its JSON-RPC batch with
/// `result = method`. Returns the URL and the batch body the stub received.
async fn stub() -> (String, tokio::task::JoinHandle<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let served = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let body = loop {
            let read = stream.read(&mut chunk).await.unwrap();
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
                break serde_json::from_slice::<Value>(&buf[end + 4..end + 4 + len]).unwrap();
            }
        };
        let answers: Vec<Value> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|item| json!({"jsonrpc": "2.0", "id": item["id"], "result": item["method"]}))
            .collect();
        let text = Value::Array(answers).to_string();
        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
            text.len()
        );
        stream.write_all(reply.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();
        body
    });
    (url, served)
}

fn chain() -> RpcChain {
    RpcChain::new(
        RpcClient::new(RpcConfig {
            timeout: Duration::from_secs(2),
            max_retries: 0,
            min_interval: Duration::ZERO,
            subject_gap_min: Duration::ZERO,
            subject_gap_max: Duration::ZERO,
            allow_loopback_http: true,
            ..RpcConfig::default()
        })
        .unwrap(),
    )
}

#[tokio::test]
async fn the_port_batch_is_one_rpc_request_with_the_calls_in_order() {
    let (url, served) = stub().await;
    let calls = [
        ChainCall::new("eth_chainId", json!([])),
        ChainCall::new("eth_blockNumber", json!([])),
    ];

    let answers = ChainSource::batch(&chain(), &url, "0xaa", &calls)
        .await
        .unwrap();

    let results: Vec<Value> = answers.into_iter().map(|answer| answer.unwrap()).collect();
    assert_eq!(
        results,
        vec![json!("eth_chainId"), json!("eth_blockNumber")]
    );
    let body = served.await.unwrap();
    let methods: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["method"].as_str().unwrap())
        .collect();
    assert_eq!(methods, ["eth_chainId", "eth_blockNumber"]);
}

#[tokio::test]
async fn the_port_call_returns_the_result_of_the_single_call() {
    let (url, served) = stub().await;

    let result = ChainSource::call(&chain(), &url, "0xaa", "eth_gasPrice", json!([]))
        .await
        .unwrap();

    assert_eq!(result, json!("eth_gasPrice"));
    assert_eq!(served.await.unwrap().as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_url_that_is_not_https_is_refused_before_any_request() {
    let refused = ChainSource::get_json(&chain(), "ftp://node.test", "/x", "0xaa").await;

    assert!(refused.is_err());
}
