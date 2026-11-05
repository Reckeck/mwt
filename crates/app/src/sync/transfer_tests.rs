//! Both transfer sessions over a real localhost socket.

use std::sync::{Arc, Mutex};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use trade_core::error::{Error, PeerSyncFailure, Result};
use trade_core::service::UnlockedVault;
use trade_core::sync::session::DeviceKey;
use trade_core::sync::transfer::{Offer, TRANSFER_PROTOCOL, sha256};

use super::transfer::{Delivered, Incoming, Me, Outgoing, Received, fingerprint_of, receive, send};

fn vault_file() -> (tempfile::TempDir, Vec<u8>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("home.wallet");
    UnlockedVault::create(path.clone(), "pw").unwrap();
    let bytes = std::fs::read(path).unwrap();
    (dir, bytes)
}

fn offer(bytes: &[u8]) -> Offer {
    Offer {
        protocol: TRANSFER_PROTOCOL,
        device_id: "laptop-id".into(),
        device_name: "Тихий Банан".into(),
        file_name: "home.wallet".into(),
        size: bytes.len() as u64,
        sha256: sha256(bytes),
    }
}

struct Run {
    received: Result<Received>,
    delivered: Result<Delivered>,
    incoming: Vec<Incoming>,
    outgoing: Vec<Outgoing>,
}

/// One transfer: the receiver answers `accept`, the sender types `codes`
/// (`None` = the code the receiver shows).
async fn run(
    accept: bool,
    codes: Vec<Option<&'static str>>,
    spoof_fingerprint: bool,
) -> (Run, Vec<u8>) {
    let (_dir, bytes) = vault_file();
    let receiver_key = DeviceKey::generate().unwrap();
    let sender_key = DeviceKey::generate().unwrap();
    let fingerprint = if spoof_fingerprint {
        fingerprint_of(&DeviceKey::generate().unwrap().public_hex())
    } else {
        fingerprint_of(&receiver_key.public_hex())
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let incoming = Arc::new(Mutex::new(Vec::new()));
    let (code_out, mut code_shown) = mpsc::unbounded_channel::<String>();
    let receiver = {
        let incoming = incoming.clone();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (decide, mut decisions) = mpsc::unbounded_channel();
            decide.send(accept).unwrap();
            let me = Me {
                device_id: "phone-id".into(),
                device_name: "Сочный Апельсин".into(),
            };
            receive(stream, &receiver_key, &me, &mut decisions, |event| {
                if let Incoming::Code(code) = &event {
                    let _ = code_out.send(code.clone());
                }
                incoming.lock().unwrap().push(event);
            })
            .await
        })
    };

    let outgoing = Arc::new(Mutex::new(Vec::new()));
    let (typed, mut codes_in) = mpsc::unbounded_channel::<String>();
    let typist = tokio::spawn(async move {
        let Some(shown) = code_shown.recv().await else {
            return;
        };
        for code in codes {
            let _ = typed.send(code.map_or_else(|| shown.clone(), str::to_string));
        }
    });
    let stream = TcpStream::connect(addr).await.unwrap();
    let delivered = {
        let outgoing = outgoing.clone();
        send(
            stream,
            &sender_key,
            offer(&bytes),
            &bytes,
            &fingerprint,
            &mut codes_in,
            |event| outgoing.lock().unwrap().push(event),
        )
        .await
    };
    typist.abort();
    let received = receiver.await.unwrap();
    let run = Run {
        received,
        delivered,
        incoming: incoming.lock().unwrap().clone(),
        outgoing: outgoing.lock().unwrap().clone(),
    };
    (run, bytes)
}

fn is(result: &Result<impl std::fmt::Debug>, kind: PeerSyncFailure) -> bool {
    matches!(result, Err(Error::PeerSync(found)) if *found == kind)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_right_code_hands_the_vault_over_byte_for_byte() {
    let (run, bytes) = run(true, vec![None], false).await;
    let received = run.received.unwrap();
    assert_eq!(received.bytes, bytes);
    assert_eq!(received.offer.device_name, "Тихий Банан");
    let delivered = run.delivered.unwrap();
    assert_eq!(delivered.device_name, "Сочный Апельсин");
    assert_eq!(delivered.device_id, "phone-id");
    assert!(matches!(run.incoming[0], Incoming::Request { .. }));
    assert!(run.outgoing.contains(&Outgoing::Accepted));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_wrong_code_can_be_typed_again() {
    let (run, bytes) = run(true, vec![Some("000000"), None], false).await;
    assert_eq!(run.received.unwrap().bytes, bytes);
    assert!(
        run.outgoing
            .contains(&Outgoing::WrongCode { attempts_left: 2 })
    );
    assert!(
        run.incoming
            .contains(&Incoming::WrongCode { attempts_left: 2 })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_wrong_codes_drop_the_request() {
    let (run, _) = run(
        true,
        vec![Some("000000"), Some("000001"), Some("000002")],
        false,
    )
    .await;
    assert!(is(&run.received, PeerSyncFailure::BadCode));
    assert!(is(&run.delivered, PeerSyncFailure::BadCode));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_declined_request_sends_nothing() {
    let (run, _) = run(false, vec![None], false).await;
    assert!(is(&run.received, PeerSyncFailure::TransferDeclined));
    assert!(is(&run.delivered, PeerSyncFailure::TransferDeclined));
    assert!(!run.outgoing.contains(&Outgoing::Accepted));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_device_with_another_key_gets_nothing() {
    let (run, _) = run(true, vec![None], true).await;
    assert!(is(&run.delivered, PeerSyncFailure::Channel));
    assert!(run.received.is_err());
}

/// A peer that completes the handshake and then never answers.
#[tokio::test(start_paused = true)]
async fn a_silent_peer_after_the_offer_ends_the_send_with_channel() {
    use super::link::{read_frame, write_frame};
    use trade_core::sync::session::Handshake;

    let (_dir, bytes) = vault_file();
    let peer_key = DeviceKey::generate().unwrap();
    let sender_key = DeviceKey::generate().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (mut reader, mut writer) = stream.into_split();
        let mut handshake = Handshake::responder(&peer_key).unwrap();
        while !handshake.is_finished() {
            if handshake.my_turn() {
                write_frame(&mut writer, &handshake.write().unwrap())
                    .await
                    .unwrap();
            } else {
                handshake
                    .read(&read_frame(&mut reader).await.unwrap())
                    .unwrap();
            }
        }
        // Keep the connection open and say nothing.
        std::future::pending::<()>().await;
    });
    let stream = TcpStream::connect(addr).await.unwrap();
    let (_typed, mut codes_in) = mpsc::unbounded_channel::<String>();
    // The outer bound makes a missing timeout fail the test instead of hanging it.
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(3600),
        send(
            stream,
            &sender_key,
            offer(&bytes),
            &bytes,
            "",
            &mut codes_in,
            |_| {},
        ),
    )
    .await
    .expect("send must give up by itself");
    peer.abort();
    assert!(is(&result, PeerSyncFailure::Channel), "{result:?}");
}
