//! Fix C, slice 6: dialing and accepting.

use std::time::Duration;

use tokio::io::AsyncReadExt;

use super::*;
use crate::sync::discovery::{MAX_INCOMING, accept_loop, connect};

/// A port that accepts and hangs up: open, but not a device of ours.
async fn not_a_device() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            drop(stream);
        }
    });
    addr
}

/// `main` serving on a port, as `discovery::start` does.
async fn serve_main(main: &TestHost) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (_stop, stopped) = tokio::sync::watch::channel(false);
    std::mem::forget(_stop);
    tokio::spawn(accept_loop(main.clone(), listener, 1, stopped));
    addr
}

fn searching(client: &TestHost) {
    client.hub().run.as_mut().unwrap().role = SyncRole::Searching;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_dial_that_fails_the_handshake_tries_the_next_address() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let _main_seen = answer(main.clone(), main_events, true);
    let _client_seen = answer(client.clone(), client_events, true);
    searching(&client);
    let bad = not_a_device().await;
    let good = serve_main(&main).await;
    let dialing = client.clone();
    tokio::spawn(async move { connect(&dialing, 1, "Laptop".into(), vec![bad, good]).await });
    until("both live", || live(&main) && live(&client)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_handshake_does_not_reset_the_dial_counters() {
    let dir = tempfile::tempdir().unwrap();
    let (_left, right) = same_vault(dir.path());
    let (client, _events) = device("Phone", right, SyncRole::Client);
    searching(&client);
    client.hub().run.as_mut().unwrap().unreached = true;
    let bad = not_a_device().await;
    tokio::time::timeout(
        Duration::from_secs(20),
        connect(&client, 1, "Laptop".into(), vec![bad]),
    )
    .await
    .expect("the dial ended");
    let hub = client.hub();
    let run = hub.run.as_ref().unwrap();
    assert!(
        run.unreached,
        "an open port that is not a device reached nobody"
    );
    assert_eq!(run.dial_failures, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connections_beyond_the_cap_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (main, _events) = device("Laptop", left, SyncRole::Main);
    let addr = serve_main(&main).await;
    let mut streams = Vec::new();
    for _ in 0..MAX_INCOMING + 4 {
        streams.push(TcpStream::connect(addr).await.unwrap());
        // In order: the first ones take the slots.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let mut closed = 0;
    for stream in &mut streams {
        let mut byte = [0u8; 1];
        if let Ok(Ok(0) | Err(_)) =
            tokio::time::timeout(Duration::from_millis(200), stream.read(&mut byte)).await
        {
            closed += 1;
        }
    }
    assert_eq!(closed, 4, "only the connections beyond the cap are dropped");
}
