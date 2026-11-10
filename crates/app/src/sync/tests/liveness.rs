//! Fix C, slices 5 and 7: liveness of the exchange and the live loop, the tail of `apply`.

use std::time::Duration;

use super::*;

/// A trusted peer driven by hand: connected and through the hellos.
struct RawPeer {
    reader: tokio::net::tcp::OwnedReadHalf,
    channel: trade_core::sync::session::Channel,
}

async fn raw_peer(main: &TestHost) -> RawPeer {
    let raw_key = DeviceKey::generate().unwrap();
    let public_key = raw_key.public_hex();
    main.with_vault(|vault| vault.sync_trust_peer("raw-device", "Raw", &public_key))
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = main.clone();
    tokio::spawn(async move {
        let (stream, peer) = listener.accept().await.unwrap();
        crate::sync::link::run(server, stream, peer, 1, false).await;
    });
    let mut raw = RawLink::dial(addr, &raw_key).await;
    let vault_id = main.with_vault(|vault| Ok(vault.doc.id.clone())).unwrap();
    raw.introduce(&vault_id, raw_identity("raw-device", "Raw"))
        .await;
    let RawLink {
        reader,
        writer,
        channel,
    } = raw;
    // The write half stays open (dropped with the test) but is never used again.
    std::mem::forget(writer);
    RawPeer { reader, channel }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_silent_peer_in_the_exchange_is_ended_with_a_timeout_bye() {
    use trade_core::sync::protocol::{Message, decode};

    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (main, _events) = device("Laptop", left, SyncRole::Main);
    let mut raw = raw_peer(&main).await;
    until("the link exists", || !main.hub().links.is_empty()).await;
    // The peer says nothing: the exchange ends by itself and says why.
    let mut bye = None;
    let ended = tokio::time::timeout(Duration::from_secs(5), async {
        while let Ok(frame) = crate::sync::link::read_frame(&mut raw.reader).await {
            if let Some(bytes) = raw.channel.open(&frame).unwrap()
                && let Message::Bye { reason } = decode(&bytes).unwrap()
            {
                bye = Some(reason);
            }
        }
    })
    .await;
    assert!(ended.is_ok(), "a silent peer kept the session open");
    assert_eq!(bye.as_deref(), Some("timeout"));
    until("the link is gone", || main.hub().links.is_empty()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_long_question_inside_the_exchange_is_not_cut_by_the_idle_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "first");
    client.0.settings.lock().unwrap().sync.confirm_first = true;
    let _main_seen = responder(main.clone(), main_events, Some(true));
    // The answer comes well after the idle limit.
    let seen = hooked(client.clone(), client_events, |_, _| {
        std::thread::sleep(EXCHANGE_IDLE_TEST_WAIT);
        Some(true)
    });
    connect(&main, &client).await;
    until("applied after the long question", || {
        comments(&client) == ["first"]
    })
    .await;
    assert!(seen.errors.lock().unwrap().is_empty());
    until("both live", || live(&main) && live(&client)).await;
}

const EXCHANGE_IDLE_TEST_WAIT: Duration = Duration::from_millis(1000);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_live_link_survives_a_question_longer_than_the_silence_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let _main_seen = responder(main.clone(), main_events, Some(true));
    let seen = hooked(client.clone(), client_events, |_, _| {
        // Longer than SILENCE_LIMIT, shorter than DECISION_TIMEOUT.
        std::thread::sleep(Duration::from_millis(1300));
        Some(true)
    });
    connect(&main, &client).await;
    until("both live", || live(&main) && live(&client)).await;
    // The news comes on the live link and is asked about there.
    client.0.settings.lock().unwrap().sync.confirm_first = true;
    for link in client.hub().links.values_mut() {
        link.applied_once = false;
    }
    let before = seen.plans();
    let started = std::time::Instant::now();
    seed(&main, "late");
    until("the question was answered", || {
        comments(&client) == ["late"]
    })
    .await;
    assert!(
        started.elapsed() > Duration::from_millis(1200),
        "no long question was asked"
    );
    assert_eq!(seen.plans(), before + 1);
    // Several ticks later the link is still there on both sides.
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert!(
        !client.hub().links.is_empty(),
        "the phone closed a live link after the question"
    );
    assert!(!main.hub().links.is_empty());
    assert!(live(&client), "the answered link is live again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closing_a_live_link_flushes_the_edits_still_waiting() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let _main_seen = answer(main.clone(), main_events, true);
    let _client_seen = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("both live", || live(&main) && live(&client)).await;
    let session = client.hub().links.keys().next().cloned().unwrap();
    // The edit is queued by the tap; the close arrives before the debounce.
    seed(&client, "last words");
    crate::sync::send(&client, &session, Order::Close);
    until("the edit reached the laptop", || {
        comments(&main) == ["last words"]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_backup_does_not_count_as_the_runs_backup() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let _main_seen = answer(main.clone(), main_events, true);
    let client_seen = answer(client.clone(), client_events, true);
    seed(&main, "first");

    // The phone cannot copy its file aside: the batch is not applied. Rotating
    // onto a non-empty directory fails for every user; file modes do not stop
    // root, which is who runs CI.
    let rotated = dir.path().join("right.wallet.presync.1.bak");
    let oldest = dir.path().join("right.wallet.presync.2.bak");
    std::fs::create_dir(&rotated).unwrap();
    std::fs::create_dir(&oldest).unwrap();
    std::fs::write(oldest.join("in the way"), b"").unwrap();
    connect(&main, &client).await;
    until("a failure is reported", || {
        !client_seen.lock().unwrap().is_empty()
    })
    .await;
    until("the failed session is over", || {
        client.hub().links.is_empty() && main.hub().links.is_empty()
    })
    .await;
    assert!(comments(&client).is_empty());
    std::fs::remove_dir_all(&rotated).unwrap();
    std::fs::remove_dir_all(&oldest).unwrap();

    // The next session in the same run still owes the backup.
    connect(&main, &client).await;
    until("applied", || comments(&client) == ["first"]).await;
    assert!(
        dir.path().join("right.wallet.presync.bak").exists(),
        "no backup was made before the batch"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_a_local_edit_with_a_live_link_counts_as_activity() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, _events) = device("Laptop", left, SyncRole::Main);
    let keys = vec!["transactions:a".to_string()];
    let _exchanging = fake_link(&host, "one", "k1", SyncPhase::Exchanging);
    crate::sync::broadcast(&host, &keys, None);
    assert!(
        host.hub().last_activity.is_none(),
        "an edit queued for a link that is not live"
    );
    let _live = fake_link(&host, "two", "k2", SyncPhase::Live);
    crate::sync::broadcast(&host, &keys, Some("two"));
    assert!(
        host.hub().last_activity.is_none(),
        "a relayed batch is not a local edit"
    );
    crate::sync::broadcast(&host, &keys, None);
    assert!(host.hub().last_activity.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_batch_that_changes_nothing_is_not_activity() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "first");
    let _main_seen = answer(main.clone(), main_events, true);
    let _client_seen = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("synced", || {
        live(&main) && live(&client) && comments(&client) == ["first"]
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    client.hub().last_activity = None;
    // The same record again, as the laptop would send it on a save.
    let key = main
        .with_vault(|vault| {
            Ok(format!(
                "transactions:{}",
                vault.doc.transactions[0].meta().id
            ))
        })
        .unwrap();
    crate::sync::broadcast(&main, &[key], None);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(client.hub().last_activity.is_none());
}
