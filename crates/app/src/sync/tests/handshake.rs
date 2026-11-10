//! Protocol 4 (catalog item 46): nothing about the device before the other
//! side's vault proof, the dialer's nonce committed before the listener's,
//! and how a protocol 3 build is refused both ways.

use serde::Serialize;
use trade_core::sync::protocol::{Hello, Message, PROTOCOL_VERSION};
use trade_core::sync::session::{commit_to, new_nonce};

use super::*;

/// Serves `host` once on localhost.
async fn serve_once(host: TestHost) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (stream, peer) = listener.accept().await.unwrap();
        super::super::link::run(host, stream, peer, 1, false).await;
    });
    addr
}

/// Everything the other side says until it closes the connection.
async fn rest(raw: &mut RawLink) -> Vec<Message> {
    let mut seen = Vec::new();
    while let Some(message) = raw.next().await {
        seen.push(message);
    }
    seen
}

/// A protocol 3 `Hello` as such a build writes it: identity included, no commit.
#[derive(Serialize)]
struct HelloV3 {
    protocol: u32,
    format: u32,
    device_id: String,
    device_name: String,
    proof: Vec<u8>,
    now_ms: u64,
    main: bool,
    prefer_main: bool,
    needs_pairing: bool,
    rank: u32,
    instance: String,
    unreached: bool,
    app_version: String,
}

fn hello_v3(name: &str, proof: Vec<u8>, main: bool) -> Vec<u8> {
    let hello = HelloV3 {
        protocol: 3,
        format: trade_core::VAULT_FORMAT_VERSION,
        device_id: format!("{name}-id"),
        device_name: name.into(),
        proof,
        now_ms: trade_core::sync::stamp::now_ms(),
        main,
        prefer_main: false,
        needs_pairing: false,
        rank: 0,
        instance: format!("{name}._mwt"),
        unreached: false,
        app_version: "0.1.0-30".into(),
    };
    rmp_serde::to_vec_named(&serde_json::json!({ "hello": hello })).unwrap()
}

fn field(payload: &Value, name: &str) -> String {
    payload
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn assert_anonymous(hello: &Hello) {
    assert_eq!(hello.device_id, "");
    assert_eq!(hello.device_name, "");
    assert_eq!(hello.instance, "");
    assert_eq!(hello.now_ms, 0);
    assert_eq!(hello.rank, 0);
    assert!(!hello.main && !hello.prefer_main && !hello.needs_pairing && !hello.unreached);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_device_without_the_vault_learns_no_name() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let main_errors = errors_only(main_events);
    let addr = serve_once(main.clone()).await;

    // A stranger on the network: any Noise key, a proof for another vault.
    let mut raw = RawLink::dial(addr, &DeviceKey::generate().unwrap()).await;
    raw.send(&Message::Hello(Hello::first(
        PROTOCOL_VERSION,
        trade_core::VAULT_FORMAT_VERSION,
        raw.channel.proof("some-other-vault"),
        String::new(),
        commit_to(&new_nonce()),
    )))
    .await;

    let seen = rest(&mut raw).await;
    assert_eq!(seen.len(), 1, "only the hello, got {seen:?}");
    let Message::Hello(hello) = &seen[0] else {
        panic!("not a hello: {:?}", seen[0]);
    };
    assert_anonymous(hello);
    assert!(hello.commit.is_empty(), "the listener commits to nothing");
    until("the refusal is reported", || {
        !main_errors.lock().unwrap().is_empty()
    })
    .await;
    assert!(main.hub().links.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_wrong_nonce_ends_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let main_errors = error_payloads(main_events);
    let raw_key = DeviceKey::generate().unwrap();
    let public_key = raw_key.public_hex();
    main.with_vault(|vault| vault.sync_trust_peer("raw-device", "Raw", &public_key))
        .unwrap();
    let vault_id = main.with_vault(|vault| Ok(vault.doc.id.clone())).unwrap();
    let addr = serve_once(main.clone()).await;

    let mut raw = RawLink::dial(addr, &raw_key).await;
    raw.send(&Message::Hello(Hello::first(
        PROTOCOL_VERSION,
        trade_core::VAULT_FORMAT_VERSION,
        raw.channel.proof(&vault_id),
        String::new(),
        commit_to(&new_nonce()),
    )))
    .await;
    let Some(Message::Hello(hello)) = raw.next().await else {
        panic!("no hello");
    };
    assert_anonymous(&hello);
    // The proof passed: now the listener tells who it is, with its nonce.
    let Some(Message::Identity(theirs)) = raw.next().await else {
        panic!("no identity");
    };
    assert_eq!(theirs.device_name, "Laptop");
    assert_eq!(theirs.nonce.len(), trade_core::sync::session::NONCE_LEN);
    // A nonce other than the committed one, chosen after seeing theirs.
    raw.send(&Message::Identity(raw_identity("raw-device", "Raw")))
        .await;

    let seen = rest(&mut raw).await;
    assert!(seen.is_empty(), "nothing after a broken commit: {seen:?}");
    until("the failure is reported", || {
        !main_errors.lock().unwrap().is_empty()
    })
    .await;
    assert_eq!(
        field(&main_errors.lock().unwrap()[0], "code"),
        "SYNC_CHANNEL"
    );
    assert!(main.hub().links.is_empty());
}

/// An old (protocol 3) client dials an updated main device: refused on the
/// version, named in the message; the old client learns no name either.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_protocol_three_dialer_is_refused_with_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let main_errors = error_payloads(main_events);
    let raw_key = DeviceKey::generate().unwrap();
    let public_key = raw_key.public_hex();
    main.with_vault(|vault| vault.sync_trust_peer("Old phone-id", "Old phone", &public_key))
        .unwrap();
    let vault_id = main.with_vault(|vault| Ok(vault.doc.id.clone())).unwrap();
    let addr = serve_once(main.clone()).await;

    let mut raw = RawLink::dial(addr, &raw_key).await;
    let proof = raw.channel.proof(&vault_id);
    raw.send_raw(&hello_v3("Old phone", proof, false)).await;

    let seen = rest(&mut raw).await;
    assert_eq!(seen.len(), 1, "only the hello, got {seen:?}");
    let Message::Hello(hello) = &seen[0] else {
        panic!("not a hello");
    };
    assert_eq!(hello.protocol, 4);
    assert_anonymous(hello);
    until("the refusal is reported", || {
        !main_errors.lock().unwrap().is_empty()
    })
    .await;
    let on_main = main_errors.lock().unwrap()[0].clone();
    assert_eq!(field(&on_main, "code"), "SYNC_VERSION_MISMATCH");
    assert_eq!(field(&on_main, "peer_name"), "Old phone");
    assert_eq!(field(&on_main, "peer_version"), "0.1.0-30");
    // Trust stays: the pair joins once the phone is updated.
    assert!(
        main.with_vault(|vault| Ok(vault.sync_state().trusted(&public_key).is_some()))
            .unwrap()
    );
}

/// An updated client dials an old (protocol 3) main device: refused on the
/// version with the old device's name, and nothing about the client is sent.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_protocol_three_listener_is_refused_with_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let (_left, right) = same_vault(dir.path());
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let client_errors = error_payloads(client_events);
    let vault_id = client.with_vault(|vault| Ok(vault.doc.id.clone())).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let dialer = client.clone();
    tokio::spawn(async move {
        let stream = TcpStream::connect(addr).await.unwrap();
        super::super::link::run(dialer, stream, addr, 1, true).await;
    });
    let (stream, _) = listener.accept().await.unwrap();
    let raw_key = DeviceKey::generate().unwrap();
    let mut raw = RawLink::handshake(stream, &raw_key, false).await;
    let proof = raw.channel.proof(&vault_id);
    raw.send_raw(&hello_v3("Old laptop", proof, true)).await;

    let seen = rest(&mut raw).await;
    assert_eq!(seen.len(), 1, "only the hello, got {seen:?}");
    let Message::Hello(hello) = &seen[0] else {
        panic!("not a hello");
    };
    assert_eq!(hello.protocol, 4);
    assert_anonymous(hello);
    until("the refusal is reported", || {
        !client_errors.lock().unwrap().is_empty()
    })
    .await;
    let on_client = client_errors.lock().unwrap()[0].clone();
    assert_eq!(field(&on_client, "code"), "SYNC_VERSION_MISMATCH");
    assert_eq!(field(&on_client, "peer_name"), "Old laptop");
    assert_eq!(field(&on_client, "peer_version"), "0.1.0-30");
}

/// Two updated devices see the same code; it is not the protocol 3 code.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn both_devices_show_one_code_bound_to_the_nonces() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let main_codes = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = main_codes.clone();
    let host = main.clone();
    let mut events = main_events;
    tokio::spawn(async move {
        while let Some((event, payload)) = events.recv().await {
            let session = field(&payload, "session_id");
            if event == super::super::OFFER_EVENT {
                super::super::send(&host, &session, Order::StartPairing);
            } else if event == super::super::PAIRING_EVENT {
                log.lock().unwrap().push(field(&payload, "code"));
            }
        }
    });
    let vault_id = main.with_vault(|vault| Ok(vault.doc.id.clone())).unwrap();
    let addr = serve_once(main.clone()).await;

    let mut raw = RawLink::dial(addr, &DeviceKey::generate().unwrap()).await;
    let mut ours = raw_identity("raw-device", "Raw");
    ours.needs_pairing = true;
    let dialer_nonce = ours.nonce.clone();
    let theirs = raw.introduce(&vault_id, ours).await;
    until("the code is shown", || {
        !main_codes.lock().unwrap().is_empty()
    })
    .await;
    let shown = main_codes.lock().unwrap()[0].clone();
    assert_eq!(
        shown,
        raw.channel.pairing_code(&dialer_nonce, &theirs.nonce)
    );
}
