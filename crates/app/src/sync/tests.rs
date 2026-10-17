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

use super::host::{Host, lock_hub};
use super::{Hub, Order, Run, SyncHub};

struct Inner {
    hub: SyncHub,
    vault: Mutex<Option<UnlockedVault>>,
    settings: Mutex<AppSettings>,
    key: DeviceKey,
    events: mpsc::UnboundedSender<(String, Value)>,
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
    fn become_main(&self) {}
    fn demote(&self) {}
    fn main_lost(&self, _generation: u64) {}
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
        rank: 0,
        next_rank: 0,
        backup_done: false,
        stop,
        announce: None,
    });
    (host, received)
}

/// Answers every question the way `accept` says («Сопрячь», the code, the
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
    let _main = answer(main.clone(), main_events, false);
    let client_errors = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("the refusal", || has(&client_errors, "SYNC_REJECTED")).await;
    assert!(comments(&main).is_empty());
    let peers = main
        .with_vault(|vault| Ok(vault.sync_state().peers.len()))
        .unwrap();
    assert_eq!(peers, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_forgotten_device_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let client_id = client.settings().device_id;
    let client_key = trade_core::sync::session::hex(&client.0.key.public);
    main.with_vault(|vault| {
        vault.sync_trust_peer(&client_id, "Phone", &client_key)?;
        vault.sync_forget_peer(&client_id)
    })
    .unwrap();
    let main_errors = answer(main.clone(), main_events, true);
    let _client = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("the refusal", || has(&main_errors, "SYNC_REVOKED")).await;
    assert!(comments(&client).is_empty());
}
