//! Two devices over a real socket on localhost: pairing, the first exchange,
//! live changes and the refusals. The app's handle is replaced by
//! [`TestHost`]; discovery (mDNS) is not involved.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use trade_core::Amount;
use trade_core::appconfig::AppSettings;
use trade_core::error::{Error, Result};
use trade_core::ipc_contract::{SyncPhase, SyncRole};
use trade_core::model::notify::NoticeRecord;
use trade_core::model::store::AssetRef;
use trade_core::model::transaction::{KindDetails, PriceStamp, TopupTx, Transaction, TxMeta};
use trade_core::service::{SyncTap, UnlockedVault};
use trade_core::sync::session::DeviceKey;

mod dialing;
mod identity;
mod liveness;

use super::host::{Host, lock_hub};
use super::{Hub, Order, Run, SyncHub};

struct Inner {
    hub: SyncHub,
    vault: Mutex<Option<UnlockedVault>>,
    settings: Mutex<AppSettings>,
    key: DeviceKey,
    events: mpsc::UnboundedSender<(String, Value)>,
    /// Generations `main_lost` was called with.
    lost: Mutex<Vec<u64>>,
    /// What this device announces as its protocol and vault format.
    versions: Mutex<(u32, u32)>,
    /// What this device announces as its app version.
    app_version: Mutex<String>,
    /// How often `become_main` / `demote` were called.
    took_main: std::sync::atomic::AtomicUsize,
    demoted: std::sync::atomic::AtomicUsize,
}

#[derive(Clone)]
struct TestHost(Arc<Inner>);

impl Host for TestHost {
    fn hub(&self) -> std::sync::MutexGuard<'_, Hub> {
        lock_hub(&self.0.hub)
    }

    fn with_vault<T>(&self, work: impl FnOnce(&mut UnlockedVault) -> Result<T>) -> Result<T> {
        let mut guard = self.0.vault.lock().unwrap();
        let vault = guard.as_mut().ok_or(Error::RequiresUnlock)?;
        work(vault)
    }

    fn settings(&self) -> AppSettings {
        self.0.settings.lock().unwrap().clone()
    }

    fn device_key(&self) -> Result<DeviceKey> {
        Ok(self.0.key.clone())
    }

    fn key_storage(&self) -> &'static str {
        "memory"
    }

    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        let value = serde_json::to_value(payload).unwrap_or(Value::Null);
        let _ = self.0.events.send((event.to_string(), value));
    }

    fn deliver(&self, _notices: Vec<NoticeRecord>) {}
    fn become_main(&self) {
        self.0
            .took_main
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn demote(&self) {
        self.0
            .demoted
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn main_lost(&self, generation: u64) {
        self.0.lost.lock().unwrap().push(generation);
    }
    fn versions(&self) -> (u32, u32) {
        *self.0.versions.lock().unwrap()
    }
    fn app_version(&self) -> String {
        self.0.app_version.lock().unwrap().clone()
    }
}

struct Tap(TestHost);

impl SyncTap for Tap {
    fn changed(&self, keys: &[String]) {
        super::broadcast(&self.0, keys, None);
    }
}

type Events = mpsc::UnboundedReceiver<(String, Value)>;

/// A device with `vault` open and sync on; its events go to the receiver.
fn device(name: &str, mut vault: UnlockedVault, role: SyncRole) -> (TestHost, Events) {
    let (events, received) = mpsc::unbounded_channel();
    let mut settings = AppSettings::default();
    settings.ensure_device_id();
    settings.device_name = Some(name.into());
    settings.sync.enabled = true;
    let host = TestHost(Arc::new(Inner {
        hub: SyncHub::default(),
        vault: Mutex::new(None),
        settings: Mutex::new(settings.clone()),
        key: DeviceKey::generate().unwrap(),
        events,
        lost: Mutex::new(Vec::new()),
        versions: Mutex::new((
            trade_core::sync::protocol::PROTOCOL_VERSION,
            trade_core::VAULT_FORMAT_VERSION,
        )),
        app_version: Mutex::new(env!("CARGO_PKG_VERSION").to_string()),
        took_main: Default::default(),
        demoted: Default::default(),
    }));
    vault.set_device(&settings.device_id);
    vault.set_sync_tap(Some(Arc::new(Tap(host.clone()))));
    *host.0.vault.lock().unwrap() = Some(vault);
    let (stop, _) = tokio::sync::watch::channel(false);
    host.hub().run = Some(Run {
        generation: 1,
        role,
        tag: String::new(),
        instance: name.into(),
        mains: Default::default(),
        others: Default::default(),
        unreached: false,
        dial_failures: 0,
        port: 0,
        started_ms: 0,
        rank: 0,
        backup_done: false,
        declined: Default::default(),
        skewed: Default::default(),
        retries: 0,
        dial_attempt: 0,
        peer_instances: Default::default(),
        pair_intent: Default::default(),
        stop,
        announce: None,
    });
    (host, received)
}

/// Answers every question the way `accept` says ("Pair", the code, the
/// list of changes); returns the error codes the device reported.
fn answer(host: TestHost, mut events: Events, accept: bool) -> Arc<Mutex<Vec<String>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        while let Some((event, payload)) = events.recv().await {
            let session = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            match event.as_str() {
                super::OFFER_EVENT => {
                    super::send(&host, &session, Order::StartPairing);
                }
                super::PAIRING_EVENT | super::PLAN_EVENT => {
                    super::send(&host, &session, Order::Decide(accept));
                }
                super::ERROR_EVENT => {
                    let code = payload
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    log.lock().unwrap().push(code.to_string());
                }
                _ => {}
            }
        }
    });
    seen
}

async fn connect(main: &TestHost, client: &TestHost) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = main.clone();
    tokio::spawn(async move {
        let (stream, peer) = listener.accept().await.unwrap();
        super::link::run(server, stream, peer, 1, false).await;
    });
    let stream = TcpStream::connect(addr).await.unwrap();
    let client = client.clone();
    tokio::spawn(async move {
        super::link::run(client, stream, addr, 1, true).await;
    });
}

/// The main device dials: the client cannot be reached (its firewall).
async fn call_back(main: &TestHost, client: &TestHost) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = client.clone();
    tokio::spawn(async move {
        let (stream, peer) = listener.accept().await.unwrap();
        super::link::run(server, stream, peer, 1, false).await;
    });
    let stream = TcpStream::connect(addr).await.unwrap();
    let main = main.clone();
    tokio::spawn(async move {
        super::link::run(main, stream, addr, 1, true).await;
    });
}

async fn until(what: &str, mut check: impl FnMut() -> bool) {
    for _ in 0..400 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out waiting for {what}");
}

fn live(host: &TestHost) -> bool {
    let hub = host.hub();
    !hub.links.is_empty()
        && hub
            .links
            .values()
            .all(|link| link.view.phase == SyncPhase::Live)
}

fn comments(host: &TestHost) -> Vec<String> {
    let mut out: Vec<String> = host
        .with_vault(|vault| {
            Ok(vault
                .doc
                .transactions
                .iter()
                .map(|tx| tx.meta().comment.clone())
                .collect())
        })
        .unwrap();
    out.sort();
    out
}

fn has(errors: &Arc<Mutex<Vec<String>>>, code: &str) -> bool {
    errors.lock().unwrap().iter().any(|seen| seen == code)
}

/// An operation in the first store, as the user would book it.
fn seed(host: &TestHost, comment: &str) {
    host.with_vault(|vault| {
        let mut meta = TxMeta::new(comment);
        meta.categories = vec!["salary".into()];
        let tx = Transaction::Topup(TopupTx {
            meta,
            store_id: vault.doc.stores[0].id.clone(),
            asset: AssetRef::fiat("USD", "US Dollar"),
            amount: Amount::parse("10")?,
            fee: None,
            from: None,
            details: KindDetails::default(),
            price: PriceStamp {
                price_usd: Some(Amount::parse("1")?),
                ..PriceStamp::default()
            },
        });
        vault.add_transaction(tx)?;
        Ok(())
    })
    .unwrap();
}

/// The same vault twice (a copied file).
fn same_vault(dir: &std::path::Path) -> (UnlockedVault, UnlockedVault) {
    let mut left = UnlockedVault::create(dir.join("left.wallet"), "pw").unwrap();
    left.persist().unwrap();
    std::fs::copy(dir.join("left.wallet"), dir.join("right.wallet")).unwrap();
    let right = UnlockedVault::open(dir.join("right.wallet"), "pw").unwrap();
    (left, right)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn devices_pair_exchange_and_stay_in_sync() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "salary on the laptop");
    seed(&client, "salary on the phone");
    let main_errors = answer(main.clone(), main_events, true);
    let client_errors = answer(client.clone(), client_events, true);

    connect(&main, &client).await;
    until("both links live", || live(&main) && live(&client)).await;

    assert_eq!(comments(&main), comments(&client));
    assert_eq!(comments(&main).len(), 2);
    // Each knows the other (and, from the other's list, itself).
    let trusted = |host: &TestHost| {
        let me = host.settings().device_id;
        host.with_vault(|vault| {
            Ok(vault
                .sync_state()
                .peers
                .iter()
                .filter(|peer| peer.device_id != me)
                .count())
        })
        .unwrap()
    };
    assert_eq!(trusted(&main), 1);
    assert_eq!(trusted(&client), 1);
    // The first batch was preceded by a backup.
    assert!(
        main.with_vault(|vault| Ok(vault.has_sync_backup()))
            .unwrap()
    );

    // A change saved on one side arrives on the other after the debounce.
    seed(&main, "rent paid live");
    until("the live change", || comments(&client).len() == 3).await;
    seed(&client, "coffee from the phone");
    until("the change back", || comments(&main).len() == 4).await;
    assert_eq!(comments(&main), comments(&client));

    assert!(
        main_errors.lock().unwrap().is_empty(),
        "{:?}",
        main_errors.lock().unwrap()
    );
    assert!(client_errors.lock().unwrap().is_empty());
    let session = main.hub().links.keys().next().cloned().unwrap();
    super::send(&main, &session, Order::Close);
    until("the link closed", || client.hub().links.is_empty()).await;
    assert!(client_errors.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_vault_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let left = UnlockedVault::create(dir.path().join("a.wallet"), "pw").unwrap();
    let right = UnlockedVault::create(dir.path().join("b.wallet"), "pw").unwrap();
    let (main, main_events) = device("A", left, SyncRole::Main);
    let (client, client_events) = device("B", right, SyncRole::Client);
    let main_errors = answer(main.clone(), main_events, true);
    let client_errors = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("both refused", || {
        has(&main_errors, "SYNC_OTHER_VAULT") && has(&client_errors, "SYNC_OTHER_VAULT")
    })
    .await;
    assert!(main.hub().links.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_declined_pairing_trusts_nobody() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&client, "stays here");
    let main_errors = answer(main.clone(), main_events, false);
    let client_errors = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("the refusal", || has(&client_errors, "SYNC_REJECTED")).await;
    until("the decliner's link ends", || main.hub().links.is_empty()).await;
    // The decliner chose it itself: no error shown, but repeated asks count.
    assert!(!has(&main_errors, "SYNC_REJECTED"));
    assert_eq!(main.hub().failures.values().map(Vec::len).sum::<usize>(), 1);
    // The asker heard a refusal: the error stays, the address takes no penalty.
    assert!(client.hub().failures.is_empty());
    assert!(comments(&main).is_empty());
    let peers = main
        .with_vault(|vault| Ok(vault.sync_state().peers.len()))
        .unwrap();
    assert_eq!(peers, 0);
}

/// "Not now" on every offer; returns the error codes reported.
fn dismiss(host: TestHost, mut events: Events) -> Arc<Mutex<Vec<String>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        while let Some((event, payload)) = events.recv().await {
            let session = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if event == super::OFFER_EVENT {
                super::send(&host, &session, Order::Dismiss);
            }
            if event == super::ERROR_EVENT {
                log.lock().unwrap().push(event);
            }
        }
    });
    seen
}

/// Collects error events and answers nothing: the offer stays open on this side.
fn errors_only(mut events: Events) -> Arc<Mutex<Vec<String>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        while let Some((event, _payload)) = events.recv().await {
            if event == super::ERROR_EVENT {
                log.lock().unwrap().push(event);
            }
        }
    });
    seen
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_put_off_device_closes_quietly_and_is_not_offered_again() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let main_errors = dismiss(main.clone(), main_events);
    // Only the main device puts the offer off. Were the client to put it off too, whichever
    // side is first closes the link, and the main device that loses the race never records
    // the decline (a put-off is local and quiet): the test raced on save speed.
    let client_errors = errors_only(client_events);
    connect(&main, &client).await;
    let client_id = client.settings().device_id;
    until("the offer put off", || {
        main.hub()
            .run
            .as_ref()
            .is_some_and(|run| run.declined.contains(&client_id))
    })
    .await;
    until("both links closed", || {
        main.hub().links.is_empty() && client.hub().links.is_empty()
    })
    .await;
    assert!(main_errors.lock().unwrap().is_empty());
    assert!(client_errors.lock().unwrap().is_empty());
    // Connecting again is closed before any offer.
    connect(&main, &client).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(main.hub().links.is_empty());
}

/// A main device and a client whose key the main device trusted and then forgot.
fn forgotten_pair(dir: &std::path::Path) -> (TestHost, Events, TestHost, Events) {
    let (left, right) = same_vault(dir);
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let client_id = client.settings().device_id;
    let client_key = trade_core::sync::session::hex(&client.0.key.public);
    main.with_vault(|vault| {
        vault.sync_trust_peer(&client_id, "Phone", &client_key)?;
        vault.sync_forget_peer(&client_id)
    })
    .unwrap();
    (main, main_events, client, client_events)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_forgotten_device_stays_untrusted_when_the_pairing_is_declined() {
    let dir = tempfile::tempdir().unwrap();
    let (main, main_events, client, client_events) = forgotten_pair(dir.path());
    seed(&main, "salary on the laptop");
    let main_errors = answer(main.clone(), main_events, false);
    let _client_errors = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("the pairing is offered and declined", || {
        has(&main_errors, "SYNC_REJECTED") || main.hub().links.is_empty()
    })
    .await;
    until("no link stays", || {
        main.hub().links.is_empty() && client.hub().links.is_empty()
    })
    .await;
    assert!(comments(&client).is_empty());
    let (trusted, revoked) = main
        .with_vault(|vault| {
            let key = trade_core::sync::session::hex(&client.0.key.public);
            Ok((
                vault.sync_state().trusted(&key).is_some(),
                vault.sync_state().is_revoked(&key),
            ))
        })
        .unwrap();
    assert!(!trusted);
    assert!(revoked);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_forgotten_device_returns_through_a_confirmed_pairing() {
    let dir = tempfile::tempdir().unwrap();
    let (main, main_events, client, client_events) = forgotten_pair(dir.path());
    seed(&main, "salary on the laptop");
    let main_errors = answer(main.clone(), main_events, true);
    let client_errors = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("both links live", || live(&main) && live(&client)).await;
    assert_eq!(comments(&client), comments(&main));
    let key = trade_core::sync::session::hex(&client.0.key.public);
    let trusted = main
        .with_vault(|vault| Ok(vault.sync_state().trusted(&key).is_some()))
        .unwrap();
    assert!(trusted);
    assert!(!has(&main_errors, "SYNC_REVOKED"));
    assert!(!has(&client_errors, "SYNC_REVOKED"));
}

#[tokio::test]
async fn the_same_error_is_announced_once_until_it_is_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, mut events) = device("Laptop", left, SyncRole::Main);
    let err = Error::Secret;
    let count = |events: &mut Events| {
        let mut found = 0;
        while let Ok((event, _)) = events.try_recv() {
            if event == super::ERROR_EVENT {
                found += 1;
            }
        }
        found
    };
    super::emit_error(&host, &err, None);
    super::emit_error(&host, &err, None);
    assert_eq!(count(&mut events), 1);
    assert!(host.hub().last_error.is_some());
    super::clear_errors(&mut host.hub());
    super::emit_error(&host, &err, None);
    assert_eq!(count(&mut events), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_main_device_calls_back_a_client_that_cannot_reach_it() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Searching);
    client.hub().run.as_mut().unwrap().unreached = true;
    seed(&main, "salary on the laptop");
    let main_errors = answer(main.clone(), main_events, true);
    let _client_errors = answer(client.clone(), client_events, true);

    call_back(&main, &client).await;
    until("both links live", || live(&main) && live(&client)).await;

    assert_eq!(comments(&client), comments(&main));
    assert_eq!(client.hub().run.as_ref().unwrap().role, SyncRole::Client);
    assert!(
        client
            .hub()
            .links
            .values()
            .all(|link| link.to_main && link.callback)
    );
    assert!(
        main.hub()
            .links
            .values()
            .all(|link| !link.to_main && link.callback)
    );
    // The client said it could not get through: the main device shows it.
    assert!(main.hub().inbound_blocked);
    assert!(main_errors.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_links_at_once_keep_the_one_the_client_dialed() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let _main_errors = answer(main.clone(), main_events, true);
    let _client_errors = answer(client.clone(), client_events, true);
    // Paired first, so both new links go straight to the exchange.
    connect(&main, &client).await;
    until("paired", || live(&main) && live(&client)).await;
    let session = main.hub().links.keys().next().cloned().unwrap();
    super::send(&main, &session, Order::Close);
    until("closed", || {
        main.hub().links.is_empty() && client.hub().links.is_empty()
    })
    .await;

    connect(&main, &client).await;
    call_back(&main, &client).await;
    until("one link each", || {
        live(&main) && live(&client) && main.hub().links.len() == 1 && client.hub().links.len() == 1
    })
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    for host in [&main, &client] {
        let hub = host.hub();
        assert_eq!(hub.links.len(), 1);
        assert!(hub.links.values().all(|link| !link.callback));
    }
}

/// A link that exists only in the hub, with its orders readable here.
fn fake_link(
    host: &TestHost,
    session: &str,
    remote_key: &str,
    phase: SyncPhase,
) -> mpsc::UnboundedReceiver<Order> {
    let (orders, received) = mpsc::unbounded_channel();
    host.hub().links.insert(
        session.into(),
        super::Link {
            view: trade_core::ipc_contract::SyncSessionView {
                session_id: session.into(),
                phase,
                ..Default::default()
            },
            orders,
            to_main: false,
            callback: false,
            instance: session.into(),
            remote_key: remote_key.into(),
            prefer_main: false,
            applied_once: false,
        },
    );
    received
}

#[tokio::test]
async fn a_change_during_the_first_exchange_is_queued_for_the_link() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, _events) = device("Laptop", left, SyncRole::Main);
    let mut exchanging = fake_link(&host, "one", "k1", SyncPhase::Exchanging);
    let mut awaiting = fake_link(&host, "two", "k2", SyncPhase::AwaitingConfirm);
    let mut origin = fake_link(&host, "origin", "k3", SyncPhase::Live);
    super::broadcast(&host, &["transactions:a".to_string()], Some("origin"));
    assert!(
        matches!(exchanging.try_recv(), Ok(Order::Changed(keys)) if keys == ["transactions:a"])
    );
    assert!(matches!(awaiting.try_recv(), Ok(Order::Changed(_))));
    assert!(origin.try_recv().is_err());
}

#[tokio::test]
async fn a_revocation_closes_that_link_and_the_others_hear_of_it() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, _events) = device("Laptop", left, SyncRole::Main);
    host.with_vault(|vault| {
        vault.sync_trust_peer("dev-gone", "Gone", "gone-key")?;
        vault.sync_trust_peer("dev-stay", "Stay", "stay-key")?;
        vault.sync_trust_peer("dev-origin", "Origin", "origin-key")?;
        vault.sync_forget_peer("dev-gone")
    })
    .unwrap();
    let mut gone = fake_link(&host, "gone", "gone-key", SyncPhase::Live);
    let mut stay = fake_link(&host, "stay", "stay-key", SyncPhase::Live);
    let mut origin = fake_link(&host, "origin", "origin-key", SyncPhase::Live);
    super::peers_changed(&host, "origin");
    assert!(matches!(gone.try_recv(), Ok(Order::Close)));
    assert!(matches!(stay.try_recv(), Ok(Order::SendPeers)));
    assert!(origin.try_recv().is_err());
}

/// Accepts the pairing and declines the list of changes.
fn accept_pairing_decline_plan(host: TestHost, mut events: Events) -> Arc<Mutex<Vec<String>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        while let Some((event, payload)) = events.recv().await {
            let session = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            match event.as_str() {
                super::OFFER_EVENT => {
                    super::send(&host, &session, Order::StartPairing);
                }
                super::PAIRING_EVENT => {
                    super::send(&host, &session, Order::Decide(true));
                }
                super::PLAN_EVENT => {
                    super::send(&host, &session, Order::Decide(false));
                }
                super::ERROR_EVENT => {
                    let code = payload
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    log.lock().unwrap().push(code.to_string());
                }
                _ => {}
            }
        }
    });
    seen
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn declining_the_list_of_changes_is_not_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "salary on the laptop");
    let main_errors = accept_pairing_decline_plan(main.clone(), main_events);
    let client_errors = accept_pairing_decline_plan(client.clone(), client_events);
    connect(&main, &client).await;
    until("the plan declined and the links gone", || {
        comments(&main).len() == 1 && main.hub().links.is_empty() && client.hub().links.is_empty()
    })
    .await;
    assert!(comments(&client).is_empty());
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(main_errors.lock().unwrap().is_empty());
    assert!(client_errors.lock().unwrap().is_empty());
    assert!(client.hub().failures.is_empty());
    assert!(main.hub().failures.is_empty());
}

// -- stage A: decline = pause, errors per peer, forget, start errors, pairing, dials ----

fn error_count(events: &mut Events) -> usize {
    let mut found = 0;
    while let Ok((event, _)) = events.try_recv() {
        if event == super::ERROR_EVENT {
            found += 1;
        }
    }
    found
}

/// What a responder saw: error codes and how many times the plan was asked.
#[derive(Clone, Default)]
struct Seen {
    errors: Arc<Mutex<Vec<String>>>,
    plans: Arc<std::sync::atomic::AtomicUsize>,
}

impl Seen {
    fn plans(&self) -> usize {
        self.plans.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Accepts every pairing; answers the list of changes with `plan` (`None` =
/// never answers, so the question times out).
fn responder(host: TestHost, mut events: Events, plan: Option<bool>) -> Seen {
    let seen = Seen::default();
    let log = seen.clone();
    tokio::spawn(async move {
        while let Some((event, payload)) = events.recv().await {
            let session = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            match event.as_str() {
                super::OFFER_EVENT => {
                    super::send(&host, &session, Order::StartPairing);
                }
                super::PAIRING_EVENT => {
                    super::send(&host, &session, Order::Decide(true));
                }
                super::PLAN_EVENT => {
                    log.plans.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if let Some(accept) = plan {
                        super::send(&host, &session, Order::Decide(accept));
                    }
                }
                super::ERROR_EVENT => {
                    let code = payload
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    log.errors.lock().unwrap().push(code.to_string());
                }
                _ => {}
            }
        }
    });
    seen
}

fn paused(host: &TestHost, device_id: &str) -> bool {
    host.with_vault(|vault| Ok(vault.sync_is_paused(device_id)))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn declining_a_plan_pauses_the_device_until_resumed() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "salary on the laptop");
    // Every new session asks about the list, not only the first pairing.
    client.0.settings.lock().unwrap().sync.confirm_first = true;
    let main_id = main.settings().device_id;
    let main_seen = responder(main.clone(), main_events, Some(true));
    let client_seen = responder(client.clone(), client_events, Some(false));
    connect(&main, &client).await;
    until("the decline pauses the laptop", || {
        paused(&client, &main_id)
    })
    .await;
    until("links gone", || {
        main.hub().links.is_empty() && client.hub().links.is_empty()
    })
    .await;
    assert_eq!(client_seen.plans(), 1);

    // Paused: the next session closes right after the handshake, quietly.
    connect(&main, &client).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(client_seen.plans(), 1);
    assert!(client.hub().links.is_empty() && main.hub().links.is_empty());
    assert!(client_seen.errors.lock().unwrap().is_empty());
    // Stage A2: the paused side stays silent, the side it refused is told
    // once and says so (and nothing else).
    assert_eq!(*main_seen.errors.lock().unwrap(), ["SYNC_PAUSED_BY_PEER"]);
    assert!(comments(&client).is_empty());

    // "Resume": the plan question comes back.
    client
        .with_vault(|vault| vault.sync_resume_peer(&main_id))
        .unwrap();
    connect(&main, &client).await;
    until("the plan is asked again", || client_seen.plans() == 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_plan_that_times_out_pauses_nobody_and_is_asked_again() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "salary on the laptop");
    // Every new session asks about the list, not only the first pairing.
    client.0.settings.lock().unwrap().sync.confirm_first = true;
    let main_id = main.settings().device_id;
    let _main_seen = responder(main.clone(), main_events, Some(true));
    let client_seen = responder(client.clone(), client_events, None);
    connect(&main, &client).await;
    until("asked", || client_seen.plans() == 1).await;
    until("closed after the timeout", || {
        client.hub().links.is_empty() && main.hub().links.is_empty()
    })
    .await;
    assert!(!paused(&client, &main_id));
    assert!(client_seen.errors.lock().unwrap().is_empty());
    connect(&main, &client).await;
    until("asked again", || client_seen.plans() == 2).await;
}

#[tokio::test]
async fn the_same_error_from_another_peer_is_announced_again() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, mut events) = device("Laptop", left, SyncRole::Main);
    let err = Error::Secret;
    super::emit_error(&host, &err, Some(("dev-a", "A")));
    super::emit_error(&host, &err, Some(("dev-a", "A")));
    assert_eq!(error_count(&mut events), 1);
    super::emit_error(&host, &err, Some(("dev-b", "B")));
    assert_eq!(error_count(&mut events), 1);
    // The error without a peer is a key of its own.
    super::emit_error(&host, &err, None);
    assert_eq!(error_count(&mut events), 1);
    super::emit_error(&host, &err, Some(("dev-b", "B")));
    assert_eq!(error_count(&mut events), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_completed_exchange_lets_that_peers_error_show_again() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let client_id = client.settings().device_id;
    let main_seen = responder(main.clone(), main_events, Some(true));
    let _client_seen = responder(client.clone(), client_events, Some(true));
    super::emit_error(&main, &Error::Secret, Some((&client_id, "Phone")));
    until("the first error", || {
        main_seen.errors.lock().unwrap().len() == 1
    })
    .await;
    connect(&main, &client).await;
    until("live", || live(&main) && live(&client)).await;
    super::emit_error(&main, &Error::Secret, Some((&client_id, "Phone")));
    until("the error shows again", || {
        main_seen.errors.lock().unwrap().len() == 2
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pairing_again_after_forgetting_works_in_the_same_run() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let main_id = main.settings().device_id;
    let _main_seen = responder(main.clone(), main_events, Some(true));
    let _client_seen = responder(client.clone(), client_events, Some(true));
    connect(&main, &client).await;
    until("paired", || live(&main) && live(&client)).await;

    // The client forgets its main device; it was seen by discovery.
    client
        .hub()
        .run
        .as_mut()
        .unwrap()
        .mains
        .insert("Laptop".into(), vec!["127.0.0.1:1".parse().unwrap()]);
    for link in client.hub().links.values_mut() {
        link.instance = "Laptop".into();
    }
    client
        .with_vault(|vault| vault.sync_forget_peer(&main_id))
        .unwrap();
    super::forget_device(&client, &main_id);
    assert!(
        !client
            .hub()
            .run
            .as_ref()
            .unwrap()
            .mains
            .contains_key("Laptop")
    );
    assert!(
        !client
            .hub()
            .run
            .as_ref()
            .unwrap()
            .declined
            .contains(&main_id)
    );
    until("links closed", || {
        main.hub().links.is_empty() && client.hub().links.is_empty()
    })
    .await;

    connect(&main, &client).await;
    until("paired again", || live(&main) && live(&client)).await;
}

#[tokio::test]
async fn a_start_error_survives_the_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, _events) = device("Laptop", left, SyncRole::Searching);
    super::emit_error(&host, &Error::Network("no port".into()), None);
    assert!(super::teardown(&host).is_some());
    assert!(host.hub().run.is_none());
    assert_eq!(host.hub().last_error.as_deref(), Some("NETWORK"));
}

#[tokio::test]
async fn a_peers_change_keeps_links_that_are_pairing() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, _events) = device("Laptop", left, SyncRole::Main);
    host.with_vault(|vault| {
        vault.sync_trust_peer("dev-back", "Back", "back-key")?;
        vault.sync_forget_peer("dev-back")?;
        vault.sync_trust_peer("dev-gone", "Gone", "gone-key")?;
        vault.sync_forget_peer("dev-gone")
    })
    .unwrap();
    // «back» is re-pairing: its key is revoked until the code is confirmed.
    let mut offer = fake_link(&host, "offer", "back-key", SyncPhase::Offer);
    let mut pairing = fake_link(&host, "pairing", "back-key", SyncPhase::Pairing);
    let mut gone = fake_link(&host, "gone", "gone-key", SyncPhase::Live);
    super::peers_changed(&host, "elsewhere");
    assert!(offer.try_recv().is_err());
    assert!(pairing.try_recv().is_err());
    assert!(matches!(gone.try_recv(), Ok(Order::Close)));
}

#[tokio::test]
async fn an_older_dial_does_not_release_a_newer_one() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, _events) = device("Phone", left, SyncRole::Client);
    host.hub().run.as_mut().unwrap().dial_attempt = 2;
    super::discovery::release_unlinked_client(&host, 1, 1);
    assert!(host.0.lost.lock().unwrap().is_empty());
    super::discovery::release_unlinked_client(&host, 1, 2);
    assert_eq!(*host.0.lost.lock().unwrap(), vec![1]);
}

// -- versions: what an older or newer build meets ------------------------------

/// Two devices that trust each other, one of them announcing other versions.
fn trusting_pair(
    dir: &std::path::Path,
    odd_versions: (u32, u32),
) -> (TestHost, Events, TestHost, Events) {
    let (left, right) = same_vault(dir);
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    for (here, there) in [(&main, &client), (&client, &main)] {
        let device_id = there.settings().device_id;
        let name = there.settings().device_name.unwrap_or_default();
        let public_key = there.0.key.public_hex();
        here.with_vault(|vault| vault.sync_trust_peer(&device_id, &name, &public_key))
            .unwrap();
    }
    *client.0.versions.lock().unwrap() = odd_versions;
    seed(&main, "only on the laptop");
    (main, main_events, client, client_events)
}

async fn assert_refused_on_both_sides(odd_versions: (u32, u32)) {
    let dir = tempfile::tempdir().unwrap();
    let (main, main_events, client, client_events) = trusting_pair(dir.path(), odd_versions);
    let main_errors = answer(main.clone(), main_events, true);
    let client_errors = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("both refused", || {
        has(&main_errors, "SYNC_VERSION_MISMATCH") && has(&client_errors, "SYNC_VERSION_MISMATCH")
    })
    .await;
    until("both links closed", || {
        main.hub().links.is_empty() && client.hub().links.is_empty()
    })
    .await;
    // Give a second, wrongly repeated announcement time to arrive.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let once = |errors: &Arc<Mutex<Vec<String>>>| {
        errors
            .lock()
            .unwrap()
            .iter()
            .filter(|code| *code == "SYNC_VERSION_MISMATCH")
            .count()
    };
    assert_eq!(once(&main_errors), 1);
    assert_eq!(once(&client_errors), 1);
    assert_eq!(main_errors.lock().unwrap().len(), 1);
    assert_eq!(client_errors.lock().unwrap().len(), 1);
    // Nothing was exchanged.
    assert!(comments(&client).is_empty());
    // Still trusted, and the address is not counted as failing.
    let main_key = main.0.key.public_hex();
    let client_key = client.0.key.public_hex();
    assert!(
        client
            .with_vault(|vault| Ok(vault.sync_state().trusted(&main_key).is_some()))
            .unwrap()
    );
    assert!(
        main.with_vault(|vault| Ok(vault.sync_state().trusted(&client_key).is_some()))
            .unwrap()
    );
    assert!(main.hub().failures.is_empty());
    assert!(client.hub().failures.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_different_protocol_is_refused_on_both_sides() {
    let current = trade_core::sync::protocol::PROTOCOL_VERSION;
    assert_refused_on_both_sides((current + 1, trade_core::VAULT_FORMAT_VERSION)).await;
}

/// Protocol 3 (a purchase is an entry of its own) does not sync with the
/// protocol 2 builds, whose item holds its purchases whole.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_protocol_two_peer_is_refused_on_both_sides() {
    assert_eq!(trade_core::sync::protocol::PROTOCOL_VERSION, 3);
    assert_refused_on_both_sides((2, trade_core::VAULT_FORMAT_VERSION)).await;
}

/// The `sync-error` payloads of a device, in order.
fn error_payloads(mut events: Events) -> Arc<Mutex<Vec<Value>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        while let Some((event, payload)) = events.recv().await {
            if event == super::ERROR_EVENT {
                log.lock().unwrap().push(payload);
            }
        }
    });
    seen
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_version_mismatch_names_the_peer_and_both_versions() {
    let dir = tempfile::tempdir().unwrap();
    let current = trade_core::sync::protocol::PROTOCOL_VERSION;
    let (main, main_events, client, client_events) =
        trusting_pair(dir.path(), (current + 1, trade_core::VAULT_FORMAT_VERSION));
    *client.0.app_version.lock().unwrap() = "9.9.9".into();
    let main_errors = error_payloads(main_events);
    let client_errors = error_payloads(client_events);
    connect(&main, &client).await;
    until("both refused", || {
        !main_errors.lock().unwrap().is_empty() && !client_errors.lock().unwrap().is_empty()
    })
    .await;
    let own = env!("CARGO_PKG_VERSION");
    let field = |payload: &Value, name: &str| {
        payload
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let on_main = main_errors.lock().unwrap()[0].clone();
    assert_eq!(field(&on_main, "code"), "SYNC_VERSION_MISMATCH");
    assert_eq!(field(&on_main, "peer_name"), "Phone");
    assert_eq!(field(&on_main, "peer_version"), "9.9.9");
    assert_eq!(field(&on_main, "own_version"), own);
    let on_client = client_errors.lock().unwrap()[0].clone();
    assert_eq!(field(&on_client, "peer_name"), "Laptop");
    assert_eq!(field(&on_client, "peer_version"), own);
    assert_eq!(field(&on_client, "own_version"), "9.9.9");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_different_format_is_refused_on_both_sides() {
    let current = trade_core::sync::protocol::PROTOCOL_VERSION;
    assert_refused_on_both_sides((current, trade_core::VAULT_FORMAT_VERSION + 1)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unknown_message_variant_closes_the_session_without_a_panic() {
    use trade_core::sync::protocol::{Hello, Message, decode, encode};
    use trade_core::sync::session::Handshake;

    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let main_errors = errors_only(main_events);
    let raw_key = DeviceKey::generate().unwrap();
    let public_key = raw_key.public_hex();
    main.with_vault(|vault| vault.sync_trust_peer("raw-device", "Raw", &public_key))
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = main.clone();
    let session = tokio::spawn(async move {
        let (stream, peer) = listener.accept().await.unwrap();
        super::link::run(server, stream, peer, 1, false).await;
    });

    // A peer by hand: the handshake and a valid hello, then a message no build knows.
    let stream = TcpStream::connect(addr).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();
    let mut handshake = Handshake::initiator(&raw_key).unwrap();
    while !handshake.is_finished() {
        if handshake.my_turn() {
            super::link::write_frame(&mut writer, &handshake.write().unwrap())
                .await
                .unwrap();
        } else {
            handshake
                .read(&super::link::read_frame(&mut reader).await.unwrap())
                .unwrap();
        }
    }
    let mut channel = handshake.finish().unwrap();
    let vault_id = main.with_vault(|vault| Ok(vault.doc.id.clone())).unwrap();
    let hello = Message::Hello(Hello {
        protocol: trade_core::sync::protocol::PROTOCOL_VERSION,
        format: trade_core::VAULT_FORMAT_VERSION,
        device_id: "raw-device".into(),
        device_name: "Raw".into(),
        proof: channel.proof(&vault_id),
        now_ms: trade_core::sync::stamp::now_ms(),
        main: false,
        prefer_main: false,
        needs_pairing: false,
        rank: 0,
        instance: String::new(),
        unreached: false,
        app_version: String::new(),
    });
    for frame in channel.seal(&encode(&hello).unwrap()).unwrap() {
        super::link::write_frame(&mut writer, &frame).await.unwrap();
    }
    // Their hello comes back.
    let their_hello = loop {
        let frame = super::link::read_frame(&mut reader).await.unwrap();
        if let Some(bytes) = channel.open(&frame).unwrap() {
            break decode(&bytes).unwrap();
        }
    };
    assert!(matches!(their_hello, Message::Hello(_)));
    let unknown =
        rmp_serde::to_vec_named(&serde_json::json!({"from_a_newer_build": {"x": 1}})).unwrap();
    for frame in channel.seal(&unknown).unwrap() {
        super::link::write_frame(&mut writer, &frame).await.unwrap();
    }

    // The session ends: the connection closes, nothing panicked, the hub is intact.
    let closed = tokio::time::timeout(Duration::from_secs(10), async {
        while super::link::read_frame(&mut reader).await.is_ok() {}
    })
    .await;
    assert!(closed.is_ok(), "the session did not close");
    let joined = tokio::time::timeout(Duration::from_secs(10), session)
        .await
        .expect("session task ended");
    assert!(joined.is_ok(), "the session panicked");
    until("the link is gone", || main.hub().links.is_empty()).await;
    // The error event travels through a channel to the collector task.
    until("the error is reported", || {
        !main_errors.lock().unwrap().is_empty()
    })
    .await;
}

// -- stage A2: the other device cut the link on purpose ------------------------

use std::sync::atomic::{AtomicUsize, Ordering};

use trade_core::ipc_contract::SyncCutReason;

/// Serves `host` on localhost; the counter says how many times it was dialed.
async fn serve(host: TestHost) -> (std::net::SocketAddr, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let dials = Arc::new(AtomicUsize::new(0));
    let count = dials.clone();
    tokio::spawn(async move {
        while let Ok((stream, peer)) = listener.accept().await {
            count.fetch_add(1, Ordering::SeqCst);
            let host = host.clone();
            tokio::spawn(async move {
                super::link::run(host, stream, peer, 1, false).await;
            });
        }
    });
    (addr, dials)
}

fn current_versions() -> (u32, u32) {
    (
        trade_core::sync::protocol::PROTOCOL_VERSION,
        trade_core::VAULT_FORMAT_VERSION,
    )
}

fn cut_reason(host: &TestHost, device_id: &str) -> Option<SyncCutReason> {
    host.with_vault(|vault| Ok(vault.sync_cut_reason(device_id)))
        .unwrap()
}

fn count_of(seen: &Seen, code: &str) -> usize {
    seen.errors
        .lock()
        .unwrap()
        .iter()
        .filter(|seen| seen.as_str() == code)
        .count()
}

/// The dialer sees the other device as a main device at `addr`.
fn sight(host: &TestHost, instance: &str, addr: std::net::SocketAddr) {
    let mut hub = host.hub();
    let run = hub.run.as_mut().unwrap();
    run.role = SyncRole::Searching;
    run.mains.insert(instance.into(), vec![addr]);
}

async fn dial(host: &TestHost, instance: &str, addr: std::net::SocketAddr) {
    host.hub().run.as_mut().unwrap().role = SyncRole::Searching;
    super::discovery::connect(host, 1, instance.into(), vec![addr]).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_paused_device_is_told_and_stops_calling() {
    let dir = tempfile::tempdir().unwrap();
    let (laptop, laptop_events, phone, phone_events) =
        trusting_pair(dir.path(), current_versions());
    let (laptop_id, phone_id) = (laptop.settings().device_id, phone.settings().device_id);
    laptop
        .with_vault(|vault| vault.sync_pause_peer(&phone_id))
        .unwrap();
    let laptop_seen = responder(laptop.clone(), laptop_events, Some(true));
    let phone_seen = responder(phone.clone(), phone_events, Some(true));
    let (addr, dials) = serve(laptop.clone()).await;
    sight(&phone, "Laptop", addr);

    dial(&phone, "Laptop", addr).await;
    until("the phone is told", || {
        cut_reason(&phone, &laptop_id) == Some(SyncCutReason::Paused)
    })
    .await;
    until("announced", || {
        count_of(&phone_seen, "SYNC_PAUSED_BY_PEER") == 1
    })
    .await;
    assert_eq!(dials.load(Ordering::SeqCst), 1);
    // The paused side itself says nothing.
    assert!(laptop_seen.errors.lock().unwrap().is_empty());

    // Several discovery rounds later nobody has dialed again, and the notice
    // did not repeat.
    for _ in 0..3 {
        dial(&phone, "Laptop", addr).await;
        assert!(super::discovery::elect_step(&phone, 1).is_none());
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(dials.load(Ordering::SeqCst), 1);
    assert_eq!(count_of(&phone_seen, "SYNC_PAUSED_BY_PEER"), 1);

    // The paused side does not call the device it paused either.
    {
        let mut hub = laptop.hub();
        let run = hub.run.as_mut().unwrap();
        run.role = SyncRole::Main;
        run.others.insert(
            "Phone".into(),
            super::discovery::Seen {
                addrs: vec![addr],
                next_call: std::time::Instant::now(),
            },
        );
    }
    assert!(super::discovery::due_callbacks(&laptop, 1).is_empty());
    laptop
        .with_vault(|vault| vault.sync_resume_peer(&phone_id))
        .unwrap();
    assert_eq!(super::discovery::due_callbacks(&laptop, 1).len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resuming_on_the_other_side_brings_the_exchange_back() {
    let dir = tempfile::tempdir().unwrap();
    let (laptop, laptop_events, phone, phone_events) =
        trusting_pair(dir.path(), current_versions());
    let (laptop_id, phone_id) = (laptop.settings().device_id, phone.settings().device_id);
    laptop
        .with_vault(|vault| vault.sync_pause_peer(&phone_id))
        .unwrap();
    let _laptop_seen = responder(laptop.clone(), laptop_events, Some(true));
    let phone_seen = responder(phone.clone(), phone_events, Some(true));
    let (addr, _dials) = serve(laptop.clone()).await;
    sight(&phone, "Laptop", addr);
    dial(&phone, "Laptop", addr).await;
    until("cut", || cut_reason(&phone, &laptop_id).is_some()).await;

    // "Resume" there; the laptop calls the phone itself.
    laptop
        .with_vault(|vault| vault.sync_resume_peer(&phone_id))
        .unwrap();
    call_back(&laptop, &phone).await;
    until("both live", || live(&laptop) && live(&phone)).await;
    assert_eq!(comments(&phone), comments(&laptop));
    until("the cut is cleared", || {
        cut_reason(&phone, &laptop_id).is_none()
    })
    .await;
    assert_eq!(count_of(&phone_seen, "SYNC_PAUSED_BY_PEER"), 1);
}

/// Collects error codes; answers the code and the plan, but not an offer: the
/// pairing starts only when `offers` is set.
fn gated(host: TestHost, mut events: Events) -> (Seen, Arc<std::sync::atomic::AtomicBool>) {
    let seen = Seen::default();
    let offers = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (log, gate) = (seen.clone(), offers.clone());
    tokio::spawn(async move {
        while let Some((event, payload)) = events.recv().await {
            let session = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            match event.as_str() {
                super::OFFER_EVENT if gate.load(Ordering::SeqCst) => {
                    super::send(&host, &session, Order::StartPairing);
                }
                super::PAIRING_EVENT | super::PLAN_EVENT => {
                    super::send(&host, &session, Order::Decide(true));
                }
                super::ERROR_EVENT => {
                    let code = payload
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    log.errors.lock().unwrap().push(code.to_string());
                }
                _ => {}
            }
        }
    });
    (seen, offers)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_forgotten_device_is_told_and_waits_for_pairing() {
    let dir = tempfile::tempdir().unwrap();
    let (laptop, laptop_events, phone, phone_events) = forgotten_pair(dir.path());
    seed(&laptop, "salary on the laptop");
    let laptop_id = laptop.settings().device_id;
    let laptop_seen = responder(laptop.clone(), laptop_events, Some(true));
    let (phone_seen, _) = gated(phone.clone(), phone_events);
    let (addr, dials) = serve(laptop.clone()).await;
    sight(&phone, "Laptop", addr);

    // Nobody asked for a pairing: the laptop says it forgot the phone.
    dial(&phone, "Laptop", addr).await;
    until("the phone is told", || {
        cut_reason(&phone, &laptop_id) == Some(SyncCutReason::Forgotten)
    })
    .await;
    // The notice reaches the listener after the cut is stored: wait for it, then make sure it is one.
    until("announced", || {
        count_of(&phone_seen, "SYNC_FORGOTTEN_BY_PEER") >= 1
    })
    .await;
    assert_eq!(count_of(&phone_seen, "SYNC_FORGOTTEN_BY_PEER"), 1);
    assert!(laptop_seen.errors.lock().unwrap().is_empty());
    for _ in 0..3 {
        dial(&phone, "Laptop", addr).await;
    }
    assert_eq!(dials.load(Ordering::SeqCst), 1);
    assert_eq!(count_of(&phone_seen, "SYNC_FORGOTTEN_BY_PEER"), 1);
    assert!(comments(&phone).is_empty());

    // "Pair again": one dial with the pairing request; the cut is over.
    super::pair_again(&phone, &laptop_id).unwrap();
    assert_eq!(cut_reason(&phone, &laptop_id), None);
    until("paired and live", || live(&laptop) && live(&phone)).await;
    assert_eq!(dials.load(Ordering::SeqCst), 2);
    assert_eq!(comments(&phone), comments(&laptop));
    assert_eq!(cut_reason(&phone, &laptop_id), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_client_cut_by_its_main_does_not_become_main() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (phone, _events) = device("Phone", left, SyncRole::Searching);
    let addr: std::net::SocketAddr = "127.0.0.1:9".parse().unwrap();
    sight(&phone, "Laptop", addr);
    phone
        .hub()
        .run
        .as_mut()
        .unwrap()
        .peer_instances
        .insert("laptop-id".into(), "Laptop".into());
    phone
        .with_vault(|vault| {
            vault
                .sync_cut_by("laptop-id", SyncCutReason::Paused)
                .map(|_| ())
        })
        .unwrap();

    assert!(super::discovery::elect_step(&phone, 1).is_none());
    assert_eq!(phone.hub().run.as_ref().unwrap().role, SyncRole::Searching);

    // Once the main device is gone from sight, the group needs a main again.
    phone.hub().run.as_mut().unwrap().mains.clear();
    assert!(super::discovery::elect_step(&phone, 1).is_none());
    assert_eq!(phone.hub().run.as_ref().unwrap().role, SyncRole::Main);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_old_peer_without_reasons_behaves_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "salary on the laptop");
    client.0.settings.lock().unwrap().sync.confirm_first = true;
    let (main_id, client_id) = (main.settings().device_id, client.settings().device_id);
    let main_seen = responder(main.clone(), main_events, Some(true));
    // The phone never answers the plan: it closes with a plain `Bye`.
    let _client_seen = responder(client.clone(), client_events, None);
    connect(&main, &client).await;
    until("closed", || {
        main.hub().links.is_empty() && client.hub().links.is_empty()
    })
    .await;
    assert_eq!(cut_reason(&main, &client_id), None);
    assert_eq!(cut_reason(&client, &main_id), None);
    assert!(main_seen.errors.lock().unwrap().is_empty());
}

// -- stage B1: records that arrive while the plan question is open ---------------------

/// Accepts every pairing; each list of changes goes to `hook` (question
/// number from 1, the summary's total), whose answer is the decision.
fn hooked(
    host: TestHost,
    mut events: Events,
    hook: impl Fn(usize, u64) -> Option<bool> + Send + 'static,
) -> Seen {
    let seen = Seen::default();
    let log = seen.clone();
    tokio::spawn(async move {
        while let Some((event, payload)) = events.recv().await {
            let session = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            match event.as_str() {
                super::OFFER_EVENT => {
                    super::send(&host, &session, Order::StartPairing);
                }
                super::PAIRING_EVENT => {
                    super::send(&host, &session, Order::Decide(true));
                }
                super::PLAN_EVENT => {
                    let total = payload["summary"]["total"].as_u64().unwrap_or(0);
                    let number = log.plans() + 1;
                    // The hook blocks (it edits the other vault and waits):
                    // off the worker, or the link task its edit wakes would
                    // sit in this worker's LIFO slot until the wait is over.
                    let answer = tokio::task::block_in_place(|| hook(number, total));
                    // Counted once the hook is through, so a test that sees
                    // the count also sees what the hook recorded.
                    log.plans.store(number, std::sync::atomic::Ordering::SeqCst);
                    if let Some(accept) = answer {
                        super::send(&host, &session, Order::Decide(accept));
                    }
                }
                super::ERROR_EVENT => {
                    let code = payload
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    log.errors.lock().unwrap().push(code.to_string());
                }
                _ => {}
            }
        }
    });
    seen
}

/// What the phone's vault held, and how big the list was, at each question.
type Asked = Arc<Mutex<Vec<(Vec<String>, u64)>>>;

/// A laptop with one operation and a phone that asks about the list; while
/// the first question is open the laptop `then` does something and the
/// phone's answer waits for it to arrive.
async fn question_with_news(
    then: impl Fn(&TestHost) + Send + 'static,
) -> (TestHost, TestHost, Seen, Asked) {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "first");
    client.0.settings.lock().unwrap().sync.confirm_first = true;
    let _main_seen = responder(main.clone(), main_events, Some(true));
    let asked: Asked = Arc::default();
    let (record, laptop, phone) = (asked.clone(), main.clone(), client.clone());
    let seen = hooked(client.clone(), client_events, move |number, total| {
        record.lock().unwrap().push((comments(&phone), total));
        if number == 1 {
            then(&laptop);
            std::thread::sleep(Duration::from_millis(400));
        }
        Some(true)
    });
    std::mem::forget(dir);
    connect(&main, &client).await;
    (main, client, seen, asked)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn records_arriving_during_the_question_are_not_applied_unseen() {
    let (_main, client, seen, asked) = question_with_news(|laptop| seed(laptop, "late")).await;
    until("a second question", || seen.plans() == 2).await;
    let asked = asked.lock().unwrap().clone();
    // Nothing was applied before the answer, and the answer took only what
    // the first list showed.
    assert!(asked[0].0.is_empty());
    assert_eq!(asked[1].0, ["first"]);
    until("everything applied", || comments(&client).len() == 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn accepting_applies_only_what_was_shown_and_asks_again_for_the_rest() {
    let (main, client, seen, asked) = question_with_news(|laptop| seed(laptop, "late")).await;
    until("both applied", || comments(&client).len() == 2).await;
    assert_eq!(comments(&client), ["first", "late"]);
    assert_eq!(seen.plans(), 2);
    // The second list holds the late record only.
    assert_eq!(asked.lock().unwrap()[1].1, 1);
    assert!(seen.errors.lock().unwrap().is_empty());
    assert_eq!(comments(&main), comments(&client));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_late_empty_diff_asks_nothing() {
    // The same record again, as the laptop would send it on a save.
    let (_main, client, seen, _asked) = question_with_news(|laptop| {
        let key = laptop
            .with_vault(|vault| {
                Ok(format!(
                    "transactions:{}",
                    vault.doc.transactions[0].meta().id
                ))
            })
            .unwrap();
        super::broadcast(laptop, &[key], None);
    })
    .await;
    until("applied", || comments(&client) == ["first"]).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(seen.plans(), 1);
    assert_eq!(comments(&client), ["first"]);
    assert!(seen.errors.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_live_link_receives_an_edit_made_just_before_a_sync_call() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let _main_seen = answer(main.clone(), main_events, true);
    let _client_seen = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("both links live", || live(&main) && live(&client)).await;

    // An edit not yet saved when another link's session asks for the
    // manifest: the call stamps it, and the save that follows must still
    // announce it on this link.
    client
        .with_vault(|vault| {
            vault.doc.stores[0].name = "Renamed on the phone".into();
            vault.sync_manifest(None)?;
            vault.persist()
        })
        .unwrap();
    until("the edit reaches the laptop without a new session", || {
        main.with_vault(|vault| Ok(vault.doc.stores[0].name == "Renamed on the phone"))
            .unwrap()
    })
    .await;
}

// -- hand-over bookkeeping and forgotten devices ------------------------------------

fn trusted(host: &TestHost, device_id: &str) -> bool {
    host.with_vault(|vault| {
        Ok(vault
            .sync_state()
            .peers
            .iter()
            .any(|peer| peer.device_id == device_id && peer.revoked.is_none()))
    })
    .unwrap()
}

#[test]
fn pending_trust_survives_a_restart() {
    use trade_core::appconfig::{load_settings, save_settings};
    let dir = tempfile::tempdir().unwrap();
    let (mut vault, _) = same_vault(dir.path());
    let path = dir.path().join("left.wallet");
    let now = 1_000_000;

    let mut settings = AppSettings::default();
    super::pending::remember(&mut settings, &path, "laptop-id", "Laptop", "key-1", now);
    save_settings(dir.path(), &settings).unwrap();

    // The process is killed: the next start reads the settings file again.
    let mut restarted = load_settings(dir.path());
    // Another file's entry is not taken.
    assert!(super::pending::take(&mut restarted, &dir.path().join("other.wallet"), now).is_empty());
    let taken = super::pending::take(&mut restarted, &path, now + 60_000);
    assert_eq!(taken.len(), 1);
    super::pending::apply(&mut vault, &taken[0]).unwrap();
    assert!(
        vault
            .sync_state()
            .peers
            .iter()
            .any(|peer| peer.device_id == "laptop-id" && peer.revoked.is_none())
    );
    // Consumed once.
    assert!(super::pending::take(&mut restarted, &path, now + 60_000).is_empty());

    // An entry nobody unlocked for a week is dropped.
    super::pending::remember(&mut restarted, &path, "laptop-id", "Laptop", "key-1", now);
    let late = now + super::pending::PENDING_TTL_MS + 1;
    assert!(super::pending::take(&mut restarted, &path, late).is_empty());
    assert!(restarted.pending_trusts.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_transfer_after_a_decline_links_again() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "salary on the laptop");
    client.0.settings.lock().unwrap().sync.confirm_first = true;
    let main_id = main.settings().device_id;
    let client_id = client.settings().device_id;
    let _main_seen = responder(main.clone(), main_events, Some(true));
    let client_seen = responder(client.clone(), client_events, Some(false));
    connect(&main, &client).await;
    until("the decline pauses the laptop", || {
        paused(&client, &main_id)
    })
    .await;
    until("links gone", || {
        main.hub().links.is_empty() && client.hub().links.is_empty()
    })
    .await;
    connect(&main, &client).await;
    until("the laptop is told", || {
        cut_reason(&main, &client_id) == Some(SyncCutReason::Paused)
    })
    .await;

    // A new transfer: the laptop sends the vault again, the phone unlocks it.
    let delivered = super::transfer::Delivered {
        device_id: client_id.clone(),
        device_name: "Phone".into(),
        device_key: client.0.key.public_hex(),
    };
    main.with_vault(|vault| super::pending::record_delivery(vault, &delivered, false).map(|_| ()))
        .unwrap();
    assert_eq!(cut_reason(&main, &client_id), None);
    assert!(!paused(&main, &client_id));
    let pending = trade_core::appconfig::PendingTrustPref {
        path: String::new(),
        device_id: main_id.clone(),
        device_name: "Laptop".into(),
        device_key: main.0.key.public_hex(),
        expires_ms: u64::MAX,
    };
    client
        .with_vault(|vault| super::pending::apply(vault, &pending))
        .unwrap();
    assert!(!paused(&client, &main_id));
    assert_eq!(cut_reason(&client, &main_id), None);

    client.0.settings.lock().unwrap().sync.confirm_first = false;
    connect(&main, &client).await;
    until("linked and exchanging again", || {
        live(&main) && live(&client) && comments(&client) == comments(&main)
    })
    .await;
    assert!(trusted(&client, &main_id));
    assert_eq!(client_seen.plans(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_forgotten_device_can_pair_again() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (laptop, laptop_events) = device("Laptop", left, SyncRole::Main);
    let (phone, phone_events) = device("Phone", right, SyncRole::Client);
    let laptop_id = laptop.settings().device_id;
    let laptop_key = laptop.0.key.public_hex();
    // The phone trusted the laptop, then the user forgot it.
    phone
        .with_vault(|vault| {
            vault.sync_trust_peer(&laptop_id, "Laptop", &laptop_key)?;
            vault.sync_forget_peer(&laptop_id)
        })
        .unwrap();
    assert!(!trusted(&phone, &laptop_id));
    seed(&laptop, "salary on the laptop");
    let _laptop_seen = responder(laptop.clone(), laptop_events, Some(true));
    let _phone_seen = responder(phone.clone(), phone_events, Some(true));
    let (addr, _dials) = serve(laptop.clone()).await;
    sight(&phone, "Laptop", addr);

    // An earlier handshake told the phone which address the laptop is.
    phone
        .hub()
        .run
        .as_mut()
        .unwrap()
        .peer_instances
        .insert(laptop_id.clone(), "Laptop".into());
    super::pair_again(&phone, &laptop_id).unwrap();
    until("paired and live", || live(&laptop) && live(&phone)).await;
    assert!(trusted(&phone, &laptop_id));
    assert_eq!(comments(&phone), comments(&laptop));
}

/// A laptop with one operation and a phone that asks about the list. While
/// the first question is open `then(laptop, phone)` runs, and the answer
/// (accept) waits `hold` for its effects. `phone_setup` runs before connecting.
async fn question_with(
    phone_setup: impl FnOnce(&TestHost),
    hold: Duration,
    then: impl Fn(&TestHost, &TestHost) + Send + 'static,
) -> (TestHost, TestHost, Seen) {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    seed(&main, "first");
    client.0.settings.lock().unwrap().sync.confirm_first = true;
    phone_setup(&client);
    let _main_seen = responder(main.clone(), main_events, Some(true));
    let (laptop, phone) = (main.clone(), client.clone());
    let seen = hooked(client.clone(), client_events, move |number, _total| {
        if number == 1 {
            then(&laptop, &phone);
            std::thread::sleep(hold);
        }
        Some(true)
    });
    std::mem::forget(dir);
    connect(&main, &client).await;
    (main, client, seen)
}

fn device_id_of(host: &TestHost) -> String {
    host.settings().device_id
}

async fn bounded(what: &str, work: impl std::future::Future<Output = ()>) {
    tokio::time::timeout(Duration::from_secs(30), work)
        .await
        .unwrap_or_else(|_| panic!("timed out: {what}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_revocation_during_the_question_is_not_lost() {
    // The laptop's list says the laptop's own key was forgotten (another
    // device did it) and the laptop sends that list while the phone asks.
    let (_main, client, seen) = question_with(
        |_| {},
        Duration::from_millis(800),
        |laptop, phone| {
            let laptop_key = trade_core::sync::session::hex(&laptop.0.key.public);
            let mut entry = phone
                .with_vault(|vault| {
                    Ok(vault
                        .sync_state()
                        .peers
                        .iter()
                        .find(|peer| peer.public_key == laptop_key)
                        .cloned())
                })
                .unwrap()
                .expect("the phone trusts the laptop after pairing");
            let stamp = trade_core::sync::stamp::Stamp::new(
                trade_core::sync::stamp::now_ms() + 60_000,
                0,
                "zzzzzzzz",
            );
            entry.revoked = Some(stamp.clone());
            entry.stamp = stamp;
            laptop
                .with_vault(|vault| vault.sync_merge_peers(&[entry]))
                .unwrap();
            let session = laptop.hub().links.keys().next().cloned().unwrap();
            super::send(laptop, &session, Order::SendPeers);
        },
    )
    .await;
    bounded("the revocation is noticed", async {
        until("the phone reports the revocation", || {
            has(&seen.errors, "SYNC_REVOKED")
        })
        .await;
    })
    .await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(
        comments(&client).is_empty(),
        "nothing is applied after the revocation"
    );
    assert!(client.hub().links.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_peer_forgotten_during_the_question_is_not_applied() {
    let (_main, client, _seen) = question_with(
        |_| {},
        Duration::from_millis(300),
        |laptop, phone| {
            let id = device_id_of(laptop);
            phone
                .with_vault(|vault| vault.sync_forget_peer(&id))
                .unwrap();
        },
    )
    .await;
    bounded("the link closes", async {
        until("the phone link is gone", || client.hub().links.is_empty()).await;
    })
    .await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(
        comments(&client).is_empty(),
        "a forgotten peer's batch is dropped"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn take_main_outside_live_is_not_dropped() {
    let demoted_early = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = demoted_early.clone();
    let (main, client, _seen) = question_with(
        |phone| phone.0.settings.lock().unwrap().sync.prefer_main = true,
        Duration::from_millis(900),
        move |laptop, _phone| {
            // The question has been open for a while by now: the laptop is
            // live and has handed the role over.
            std::thread::sleep(Duration::from_millis(700));
            if laptop.0.demoted.load(std::sync::atomic::Ordering::SeqCst) > 0 {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        },
    )
    .await;
    bounded("the phone takes the main role", async {
        until("the phone became main", || {
            client.0.took_main.load(std::sync::atomic::Ordering::SeqCst) == 1
        })
        .await;
    })
    .await;
    bounded("the laptop steps down", async {
        until("the laptop demoted", || {
            main.0.demoted.load(std::sync::atomic::Ordering::SeqCst) == 1
        })
        .await;
    })
    .await;
    assert!(
        !demoted_early.load(std::sync::atomic::Ordering::SeqCst),
        "the main device must not step down before the client answered"
    );
}

// -- stage T: time and presence ---------------------------------------------------

fn peer_of(session: &str, device_id: &str) -> super::link::Peer {
    super::link::Peer {
        session_id: session.into(),
        device_id: device_id.into(),
        name: "Peer".into(),
        remote_key: String::new(),
        vault_id: String::new(),
        take_main_heard: Default::default(),
        take_main_ordered: Default::default(),
        stable_reports: Default::default(),
    }
}

fn unreliable(host: &TestHost, session: &str) -> bool {
    host.hub()
        .links
        .get(session)
        .is_some_and(|link| link.view.unreliable)
}

fn single_device_host() -> (tempfile::TempDir, TestHost) {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, _events) = device("Laptop", left, SyncRole::Main);
    (dir, host)
}

#[tokio::test]
async fn relayed_future_stamp_does_not_mark_relay_unreliable() {
    use trade_core::sync::RecordState;
    use trade_core::sync::session::ClockWatch;
    use trade_core::sync::stamp::{Stamp, device_tag, now_ms};

    let (_dir, host) = single_device_host();
    let _orders = fake_link(&host, "relay", "key", SyncPhase::Live);
    let peer = peer_of("relay", "relay-device-1234567890");
    let watch = ClockWatch::new(now_ms(), now_ms());
    let future = now_ms() + trade_core::sync::session::SKEW_FINE_MS * 4;
    let stamped = |device: &str| {
        vec![(
            "key".to_string(),
            RecordState {
                tombstone: Some(Stamp::new(future, 0, &device_tag(device))),
                ..Default::default()
            },
        )]
    };

    // Authored by a third device: the relay is not to blame.
    super::link::check_stamps(&host, &peer, &watch, &stamped("third-device-abcdefgh"));
    assert!(!unreliable(&host, "relay"));
    // Authored by the relay itself.
    super::link::check_stamps(&host, &peer, &watch, &stamped("relay-device-1234567890"));
    assert!(unreliable(&host, "relay"));
}

#[tokio::test]
async fn unreliable_clears_after_stable_time_reports() {
    use trade_core::sync::session::{ClockWatch, DRIFT_MS};

    let (_dir, host) = single_device_host();
    let _orders = fake_link(&host, "link", "key", SyncPhase::Live);
    let peer = peer_of("link", "peer-device-1234567890");
    let base = 1_000_000_000u64;
    let mut watch = ClockWatch::new(base, base);

    // A clock that went backwards marks the link.
    let mut local = base + 30_000;
    super::link::time_report(&host, &peer, &mut watch, base - 1, local);
    assert!(unreliable(&host, "link"));
    // Reports back within the bound: two are not enough, the third clears.
    for step in 1..=3u64 {
        local += 30_000;
        super::link::time_report(&host, &peer, &mut watch, local + 1_000, local);
        let expected = step < super::link::STABLE_TIME_REPORTS as u64;
        assert_eq!(unreliable(&host, "link"), expected, "after report {step}");
    }
    // A new drift sets it again.
    local += 30_000;
    super::link::time_report(&host, &peer, &mut watch, local + DRIFT_MS + 5_000, local);
    assert!(unreliable(&host, "link"));
}

#[tokio::test]
async fn clock_skew_refusal_is_not_retried() {
    use trade_core::sync::protocol::{Hello, Message, encode};
    use trade_core::sync::session::{Handshake, SKEW_REFUSE_MS};

    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let main_errors = errors_only(main_events);
    let raw_key = DeviceKey::generate().unwrap();
    let public_key = raw_key.public_hex();
    main.with_vault(|vault| vault.sync_trust_peer("raw-device", "Raw", &public_key))
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = main.clone();
    let session = tokio::spawn(async move {
        let (stream, peer) = listener.accept().await.unwrap();
        super::link::run(server, stream, peer, 1, false).await;
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();
    let mut handshake = Handshake::initiator(&raw_key).unwrap();
    while !handshake.is_finished() {
        if handshake.my_turn() {
            super::link::write_frame(&mut writer, &handshake.write().unwrap())
                .await
                .unwrap();
        } else {
            handshake
                .read(&super::link::read_frame(&mut reader).await.unwrap())
                .unwrap();
        }
    }
    let mut channel = handshake.finish().unwrap();
    let vault_id = main.with_vault(|vault| Ok(vault.doc.id.clone())).unwrap();
    let hello = Message::Hello(Hello {
        protocol: trade_core::sync::protocol::PROTOCOL_VERSION,
        format: trade_core::VAULT_FORMAT_VERSION,
        device_id: "raw-device".into(),
        device_name: "Raw".into(),
        proof: channel.proof(&vault_id),
        now_ms: trade_core::sync::stamp::now_ms() + SKEW_REFUSE_MS + 3_600_000,
        main: false,
        prefer_main: false,
        needs_pairing: false,
        rank: 0,
        instance: "skewed-instance".into(),
        unreached: false,
        app_version: String::new(),
    });
    for frame in channel.seal(&encode(&hello).unwrap()).unwrap() {
        super::link::write_frame(&mut writer, &frame).await.unwrap();
    }
    let joined = tokio::time::timeout(Duration::from_secs(10), session)
        .await
        .expect("session task ended");
    assert!(joined.is_ok());
    until("the refusal is reported", || {
        !main_errors.lock().unwrap().is_empty()
    })
    .await;

    // The instance is not dialed or called back any more.
    assert!(super::discovery::skipped_instances(&main, false).contains("skewed-instance"));
    assert!(super::discovery::skipped_instances(&main, true).contains("skewed-instance"));
    main.hub().run.as_mut().unwrap().role = SyncRole::Searching;
    super::discovery::connect(&main, 1, "skewed-instance".into(), vec![addr]).await;
    assert_eq!(main.hub().run.as_ref().unwrap().role, SyncRole::Searching);
    assert_eq!(
        main_errors.lock().unwrap().len(),
        1,
        "the error is shown once"
    );

    // A manual retry clears it.
    main.hub().run.as_mut().unwrap().skewed.clear();
    assert!(!super::discovery::skipped_instances(&main, false).contains("skewed-instance"));
}

#[tokio::test]
async fn presence_is_resent_when_a_link_closes() {
    let (_dir, host) = single_device_host();
    let mut stays = fake_link(&host, "stays", "key-a", SyncPhase::Live);
    let _leaves = fake_link(&host, "leaves", "key-b", SyncPhase::Live);
    host.hub().links.get_mut("leaves").unwrap().view.focused = true;

    super::link::forget_link(&host, "leaves");

    let order = tokio::time::timeout(Duration::from_secs(5), stays.recv())
        .await
        .expect("an order arrives");
    assert!(matches!(order, Some(Order::Presence(false))));
}

// -- stage T: roles and time (slice 2) --------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ex_main_connects_to_the_new_main_after_handover() {
    let (_dir, host) = single_device_host();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    host.hub()
        .run
        .as_mut()
        .unwrap()
        .mains
        .insert("new-main".into(), vec![addr]);

    super::discovery::demote(&host);

    assert_eq!(host.hub().run.as_ref().unwrap().role, SyncRole::Searching);
    // The election follows the handover on its own, no discovery event needed.
    let accepted = tokio::time::timeout(Duration::from_secs(10), listener.accept()).await;
    assert!(accepted.is_ok(), "the ex-main dials the new main");
    assert_eq!(host.hub().run.as_ref().unwrap().role, SyncRole::Client);
}

#[test]
fn clients_do_not_all_become_main_after_a_failed_dial() {
    use super::discovery::retry_delay;
    let base = Duration::from_secs(14);
    // Same rank, different instances: different delays, inside [base, base + 3 s).
    let delays: Vec<Duration> = ["phone-a1b2", "tablet-c3d4", "laptop-e5f6", "watch-0718"]
        .iter()
        .map(|instance| retry_delay(instance, 1, base))
        .collect();
    for delay in &delays {
        assert!(*delay >= base + Duration::from_secs(2));
        assert!(*delay < base + Duration::from_secs(2) + Duration::from_millis(3000));
    }
    let distinct: std::collections::BTreeSet<_> = delays.iter().collect();
    assert!(
        distinct.len() > 1,
        "instances must not elect at the same moment"
    );
    // Deterministic per instance.
    assert_eq!(retry_delay("phone-a1b2", 1, base), delays[0]);
    // The rank still orders the queue.
    assert!(retry_delay("phone-a1b2", 2, base) > retry_delay("phone-a1b2", 1, base));
}

#[tokio::test]
async fn searching_with_a_main_in_sight_runs_no_jobs() {
    let (_dir, host) = single_device_host();
    let addr: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();
    let set_role = |role: SyncRole| host.hub().run.as_mut().unwrap().role = role;
    set_role(SyncRole::Searching);
    assert!(super::runs_jobs(&host), "nobody in sight: jobs run");
    host.hub()
        .run
        .as_mut()
        .unwrap()
        .mains
        .insert("seen-main".into(), vec![addr]);
    assert!(!super::runs_jobs(&host), "a main in sight: no jobs");
    // A main that skipped this device (clock) does not count as in sight.
    host.hub()
        .run
        .as_mut()
        .unwrap()
        .skewed
        .insert("seen-main".into());
    assert!(super::runs_jobs(&host));
    set_role(SyncRole::Main);
    assert!(super::runs_jobs(&host));
    set_role(SyncRole::Client);
    assert!(!super::runs_jobs(&host));
}

#[tokio::test]
async fn rank_does_not_grow_with_reconnects() {
    let (_dir, host) = single_device_host();
    let _first = fake_link(&host, "one", "k1", SyncPhase::Live);
    for _ in 0..10 {
        assert_eq!(super::link::client_rank(&host.hub()), 1);
    }
    let _second = fake_link(&host, "two", "k2", SyncPhase::Live);
    let _pending = fake_link(&host, "three", "k3", SyncPhase::Exchanging);
    assert_eq!(super::link::client_rank(&host.hub()), 2);
    let mut keep = Vec::new();
    for number in 0..8 {
        keep.push(fake_link(
            &host,
            &format!("more{number}"),
            "k",
            SyncPhase::Live,
        ));
    }
    assert_eq!(super::link::client_rank(&host.hub()), super::link::MAX_RANK);
    // An old main hands out an ever larger rank: the client caps it.
    assert_eq!(super::link::capped_rank(40), super::link::MAX_RANK);
    assert_eq!(super::link::capped_rank(2), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn handover_during_an_open_question_keeps_one_main() {
    use std::sync::atomic::Ordering;
    let both_down = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (main, client, _seen) = question_with(
        |phone| phone.0.settings.lock().unwrap().sync.prefer_main = true,
        Duration::from_millis(900),
        |_, _| {},
    )
    .await;
    // Sample while the question is open: the main steps down only after the
    // client took the role, so some device is always main.
    let watcher = {
        let (main, client, flag) = (main.clone(), client.clone(), both_down.clone());
        tokio::spawn(async move {
            for _ in 0..600 {
                let demoted = main.0.demoted.load(Ordering::SeqCst) > 0;
                let took = client.0.took_main.load(Ordering::SeqCst) > 0;
                if demoted && !took {
                    flag.store(true, Ordering::SeqCst);
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
    };
    bounded("the handover completes", async {
        until("both sides handed over", || {
            client.0.took_main.load(Ordering::SeqCst) == 1
                && main.0.demoted.load(Ordering::SeqCst) == 1
        })
        .await;
    })
    .await;
    watcher.await.unwrap();
    assert!(
        !both_down.load(Ordering::SeqCst),
        "never two non-main devices"
    );
    assert_eq!(main.0.took_main.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_forgotten_main_is_not_dialed_again() {
    let (_dir, host) = single_device_host();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    host.with_vault(|vault| vault.sync_trust_peer("dev-main", "Main", "main-key"))
        .unwrap();
    {
        let mut hub = host.hub();
        let run = hub.run.as_mut().unwrap();
        run.role = SyncRole::Searching;
        run.peer_instances
            .insert("dev-main".into(), "inst-main".into());
        run.mains.insert("inst-main".into(), vec![addr]);
        run.others.insert(
            "inst-main".into(),
            super::discovery::Seen {
                addrs: vec![addr],
                next_call: std::time::Instant::now(),
            },
        );
    }
    host.with_vault(|vault| vault.sync_forget_peer("dev-main"))
        .unwrap();
    super::forget_device(&host, "dev-main");
    {
        let hub = host.hub();
        let run = hub.run.as_ref().unwrap();
        assert!(run.mains.is_empty() && run.others.is_empty());
    }
    // Discovery shows it again: it is skipped, no dial, no election.
    host.hub()
        .run
        .as_mut()
        .unwrap()
        .mains
        .insert("inst-main".into(), vec![addr]);
    assert!(super::discovery::skipped_instances(&host, false).contains("inst-main"));
    assert!(super::discovery::elect_step(&host, 1).is_none());
    super::discovery::connect(&host, 1, "inst-main".into(), vec![addr]).await;
    let dialed = tokio::time::timeout(Duration::from_millis(500), listener.accept()).await;
    assert!(dialed.is_err(), "a forgotten device is not dialed");
    assert_eq!(host.hub().run.as_ref().unwrap().role, SyncRole::Searching);

    // Pairing it again lifts the skip.
    host.with_vault(|vault| vault.sync_trust_peer("dev-main", "Main", "main-key"))
        .unwrap();
    assert!(!super::discovery::skipped_instances(&host, false).contains("inst-main"));
}
