//! Finding the devices that have the same vault open, and who of them is the
//! main device.
//!
//! Each device listens on an ephemeral TCP port and announces it over mDNS
//! with only a daily tag of its vault (`session::discovery_tag`) and whether
//! it is the main device. Without a main device in sight for a few seconds a
//! device becomes main; the others connect to it. A device whose firewall
//! drops incoming connections can still be main: it calls back the devices
//! it sees but has no link with.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tauri::AppHandle;
use tokio::net::{TcpListener, TcpStream};
use trade_core::error::{Error, Result};
use trade_core::ipc_contract::SyncRole;
use trade_core::sync::session::{day_of, discovery_tag};
use trade_core::sync::stamp::now_ms;

use super::{Order, Run, hub};

pub const SERVICE: &str = "_mwt-sync._tcp.local.";
/// The port sync listens on, so a firewall rule can name it; a random one
/// when it is taken (another copy of the app on this computer).
pub const SYNC_PORT: u16 = 53318;
/// How long a device looks for a main device before becoming one.
const ELECTION_WAIT: Duration = Duration::from_secs(3);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the main device waits for a device it sees to connect before it
/// calls that device itself, and how often it looks.
const CALLBACK_AFTER: Duration = Duration::from_secs(6);
const CALLBACK_EVERY: Duration = Duration::from_secs(5);
const CALLBACK_RETRY: Duration = Duration::from_secs(30);
/// Failed connections to a main device before it is taken for gone.
const MAX_DIAL_FAILURES: u32 = 3;

/// A device of the group that is not the main one, as discovery saw it.
pub(crate) struct Seen {
    addrs: Vec<SocketAddr>,
    next_call: Instant,
}

/// This device's address on the local network: the one multicast leaves from.
pub(crate) fn local_address() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("224.0.0.251", 5353)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

/// [`SYNC_PORT`], waiting a moment for a previous run to let go of it.
async fn listen() -> Result<TcpListener> {
    for _ in 0..10 {
        if let Ok(listener) = TcpListener::bind(("0.0.0.0", SYNC_PORT)).await {
            return Ok(listener);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(TcpListener::bind(("0.0.0.0", 0)).await?)
}

/// The mDNS announcement of this device.
pub(crate) struct Announcer {
    daemon: ServiceDaemon,
    instance: String,
    port: u16,
    tag: String,
}

impl Announcer {
    fn register(&self, main: bool) -> Result<()> {
        let props = [
            ("t", self.tag.as_str()),
            ("m", if main { "1" } else { "0" }),
            ("v", "1"),
        ];
        let info = ServiceInfo::new(
            SERVICE,
            &self.instance,
            &format!("{}.local.", self.instance),
            "",
            self.port,
            &props[..],
        )
        .map_err(|err| Error::Network(err.to_string()))?
        .enable_addr_auto();
        self.daemon
            .register(info)
            .map_err(|err| Error::Network(err.to_string()))
    }

    pub(crate) fn shutdown(self) {
        let fullname = format!("{}.{SERVICE}", self.instance);
        let _ = self.daemon.unregister(&fullname);
        let _ = self.daemon.shutdown();
    }
}

fn random_instance() -> String {
    let bytes: [u8; 6] = rand::random();
    trade_core::sync::session::hex(&bytes)
}

fn current(app: &AppHandle, generation: u64) -> bool {
    hub(app)
        .run
        .as_ref()
        .is_some_and(|run| run.generation == generation)
}

pub(crate) async fn start(app: &AppHandle, generation: u64, vault_id: &str) -> Result<()> {
    let listener = listen().await?;
    let port = listener.local_addr()?.port();
    let tag = discovery_tag(vault_id, day_of(now_ms()));
    let instance = random_instance();
    let daemon = ServiceDaemon::new().map_err(|err| Error::Network(err.to_string()))?;
    let browse = daemon
        .browse(SERVICE)
        .map_err(|err| Error::Network(err.to_string()))?;
    let announcer = Announcer {
        daemon,
        instance: instance.clone(),
        port,
        tag: tag.clone(),
    };
    announcer.register(false)?;

    let (stop, stopped) = tokio::sync::watch::channel(false);
    {
        let mut hub = hub(app);
        if hub.generation != generation {
            announcer.shutdown();
            return Ok(());
        }
        hub.run = Some(Run {
            generation,
            role: SyncRole::Searching,
            tag: tag.clone(),
            instance: instance.clone(),
            mains: Default::default(),
            others: Default::default(),
            unreached: false,
            dial_failures: 0,
            port,
            started_ms: i64::try_from(now_ms()).unwrap_or(i64::MAX),
            rank: 0,
            next_rank: 0,
            backup_done: false,
            declined: Default::default(),
            retries: 0,
            stop,
            announce: Some(announcer),
        });
    }

    // Incoming connections.
    {
        let app = app.clone();
        let mut stopped = stopped.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::select! {
                    _ = stopped.changed() => break,
                    accepted = listener.accept() => {
                        let Ok((stream, addr)) = accepted else { continue };
                        if super::blocked(&app, addr.ip()) {
                            continue;
                        }
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            super::link::run(app, stream, addr, generation, false).await;
                        });
                    }
                }
            }
        });
    }

    // Discovery events come on a blocking channel: a thread forwards them.
    {
        let app = app.clone();
        let mine = instance.clone();
        std::thread::spawn(move || {
            while let Ok(event) = browse.recv() {
                if !current(&app, generation) {
                    break;
                }
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if info.get_property_val_str("t") != Some(tag.as_str()) {
                            continue;
                        }
                        let Some(other) = instance_of(&info.fullname) else {
                            continue;
                        };
                        if other == mine {
                            continue;
                        }
                        let main = info.get_property_val_str("m") == Some("1");
                        let mut addrs: Vec<SocketAddr> = info
                            .addresses
                            .iter()
                            .map(|ip| SocketAddr::new(ip.to_ip_addr(), info.port))
                            .collect();
                        addrs.sort_by_key(|addr| !addr.is_ipv4());
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            found(&app, generation, other, addrs, main).await;
                        });
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        if let Some(other) = instance_of(&fullname)
                            && let Some(run) = hub(&app).run.as_mut()
                        {
                            run.mains.remove(&other);
                            run.others.remove(&other);
                        }
                        super::emit_status(&app);
                    }
                    _ => {}
                }
            }
        });
    }

    // The main device calls back the devices that do not reach it.
    {
        let app = app.clone();
        let mut stopped = stopped.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::select! {
                    _ = stopped.changed() => break,
                    () = tokio::time::sleep(CALLBACK_EVERY) => {}
                }
                for addrs in due_callbacks(&app, generation) {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        callback(&app, generation, addrs).await;
                    });
                }
            }
        });
    }

    // Election and the daily tag change.
    {
        let app = app.clone();
        let mut stopped = stopped;
        let vault_id = vault_id.to_string();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(ELECTION_WAIT).await;
            elect(&app, generation).await;
            loop {
                tokio::select! {
                    _ = stopped.changed() => break,
                    () = tokio::time::sleep(Duration::from_secs(600)) => {
                        let fresh = discovery_tag(&vault_id, day_of(now_ms()));
                        let stale = hub(&app).run.as_ref().is_some_and(|run| run.tag != fresh);
                        if stale {
                            super::restart(&app);
                            break;
                        }
                    }
                }
            }
        });
    }
    super::emit_status(app);
    Ok(())
}

fn instance_of(fullname: &str) -> Option<String> {
    fullname
        .strip_suffix(SERVICE)
        .and_then(|head| head.strip_suffix('.'))
        .map(str::to_string)
}

/// A device with the same vault showed up (or changed its main flag).
async fn found(
    app: &AppHandle,
    generation: u64,
    other: String,
    addrs: Vec<SocketAddr>,
    main: bool,
) {
    let connect_to = {
        let mut hub = hub(app);
        let Some(run) = hub.run.as_mut().filter(|run| run.generation == generation) else {
            return;
        };
        if !main {
            run.mains.remove(&other);
            let next_call = Instant::now() + CALLBACK_AFTER;
            run.others
                .entry(other)
                .and_modify(|seen| seen.addrs.clone_from(&addrs))
                .or_insert(Seen { addrs, next_call });
            drop(hub);
            super::emit_status(app);
            return;
        }
        run.others.remove(&other);
        run.mains.insert(other.clone(), addrs.clone());
        match run.role {
            SyncRole::Searching => Some(addrs),
            // Two main devices: the one with the larger instance steps down.
            SyncRole::Main if run.instance > other => {
                demote_locked(&mut hub);
                Some(addrs)
            }
            _ => None,
        }
    };
    if let Some(addrs) = connect_to {
        connect(app, generation, other, addrs).await;
    }
}

/// Become a client-to-be: stop announcing as main and drop every link.
fn demote_locked(hub: &mut super::Hub) {
    if let Some(run) = hub.run.as_mut() {
        run.role = SyncRole::Searching;
        if let Some(announce) = &run.announce {
            let _ = announce.register(false);
        }
    }
    for link in std::mem::take(&mut hub.links).into_values() {
        let _ = link.orders.send(Order::Close);
    }
}

/// Hand the main role over (the preferred device is here).
pub(crate) fn demote(app: &AppHandle) {
    demote_locked(&mut hub(app));
    super::emit_status(app);
}

async fn connect(app: &AppHandle, generation: u64, instance: String, addrs: Vec<SocketAddr>) {
    {
        let mut hub = hub(app);
        let Some(run) = hub.run.as_mut().filter(|run| run.generation == generation) else {
            return;
        };
        // Already connected (or connecting) to a main device.
        if run.role != SyncRole::Searching {
            return;
        }
        run.role = SyncRole::Client;
    }
    super::emit_status(app);
    for addr in addrs {
        if let Ok(Ok(stream)) =
            tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await
        {
            if let Some(run) = hub(app).run.as_mut() {
                run.unreached = false;
                run.dial_failures = 0;
            }
            super::link::run(app.clone(), stream, addr, generation, true).await;
            return;
        }
    }
    // Unreachable: most likely a firewall there, so wait for the main device
    // to call back; after a few tries take it for gone and look again.
    {
        let mut guard = hub(app);
        let hub = &mut *guard;
        let called_back = hub.links.values().any(|link| link.to_main);
        let Some(run) = hub.run.as_mut().filter(|run| run.generation == generation) else {
            return;
        };
        if called_back {
            return;
        }
        run.role = SyncRole::Searching;
        run.unreached = true;
        run.dial_failures += 1;
        if run.dial_failures >= MAX_DIAL_FAILURES {
            run.mains.remove(&instance);
            run.dial_failures = 0;
        }
    }
    super::emit_status(app);
    elect_later(
        app,
        generation,
        CALLBACK_AFTER + CALLBACK_EVERY + ELECTION_WAIT,
    );
}

/// On the main device: the devices seen for a while with no link to them.
fn due_callbacks(app: &AppHandle, generation: u64) -> Vec<Vec<SocketAddr>> {
    let mut guard = hub(app);
    let hub = &mut *guard;
    let linked: std::collections::BTreeSet<&str> = hub
        .links
        .values()
        .map(|link| link.instance.as_str())
        .collect();
    let Some(run) = hub
        .run
        .as_mut()
        .filter(|run| run.generation == generation && run.role == SyncRole::Main)
    else {
        return Vec::new();
    };
    let now = Instant::now();
    run.others
        .iter_mut()
        .filter(|(instance, seen)| !linked.contains(instance.as_str()) && seen.next_call <= now)
        .map(|(_, seen)| {
            seen.next_call = now + CALLBACK_RETRY;
            seen.addrs.clone()
        })
        .collect()
}

/// Connect to a device of the group from the main device (a failure is
/// retried after [`CALLBACK_RETRY`]).
async fn callback(app: &AppHandle, generation: u64, addrs: Vec<SocketAddr>) {
    for addr in addrs {
        if let Ok(Ok(stream)) =
            tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await
        {
            super::link::run(app.clone(), stream, addr, generation, true).await;
            return;
        }
    }
}

/// [`elect`] later, on its own task (breaks the `connect` ↔ `elect` cycle).
fn elect_later(app: &AppHandle, generation: u64, after: Duration) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(after).await;
        let next = elect_step(&app, generation);
        if let Some((instance, addrs)) = next {
            connect(&app, generation, instance, addrs).await;
        }
    });
}

/// No main device in sight: become one; one in sight: connect to it.
pub(crate) async fn elect(app: &AppHandle, generation: u64) {
    if let Some((instance, addrs)) = elect_step(app, generation) {
        connect(app, generation, instance, addrs).await;
    }
}

/// The decision part of [`elect`]: `Some` = connect there.
fn elect_step(app: &AppHandle, generation: u64) -> Option<(String, Vec<SocketAddr>)> {
    let next = {
        let mut hub = hub(app);
        let run = hub
            .run
            .as_mut()
            .filter(|run| run.generation == generation)?;
        if run.role != SyncRole::Searching {
            return None;
        }
        match run.mains.iter().next() {
            Some((instance, addrs)) => Some((instance.clone(), addrs.clone())),
            None => {
                become_main_locked(run);
                None
            }
        }
    };
    if next.is_none() {
        super::emit_status(app);
    }
    next
}

fn become_main_locked(run: &mut Run) {
    run.role = SyncRole::Main;
    run.next_rank = 0;
    if let Some(announce) = &run.announce {
        let _ = announce.register(true);
    }
}

/// Take the main role now (handed over by the previous main device).
pub(crate) fn become_main(app: &AppHandle) {
    {
        let mut hub = hub(app);
        if let Some(run) = hub.run.as_mut() {
            become_main_locked(run);
        }
    }
    super::emit_status(app);
}

/// The link to the main device closed: wait our turn, then take over or
/// connect to whoever did.
pub(crate) fn main_lost(app: &AppHandle, generation: u64) {
    let (rank, retries) = {
        let mut guard = hub(app);
        let hub = &mut *guard;
        // A twin of a live link closed: the main device is still here.
        let still_linked = hub.links.values().any(|link| link.to_main);
        let Some(run) = hub.run.as_mut().filter(|run| run.generation == generation) else {
            return;
        };
        if still_linked {
            return;
        }
        run.role = SyncRole::Searching;
        run.retries = run.retries.saturating_add(1);
        (run.rank, run.retries)
    };
    super::emit_status(app);
    // A session that keeps failing (declined, refused, unreachable) is not
    // retried every few seconds: 5 s, 10 s, 20 s… up to 5 minutes.
    let backoff = if retries > 1 {
        5_000u64
            .saturating_mul(1 << (retries - 2).min(6))
            .min(300_000)
    } else {
        0
    };
    elect_later(
        app,
        generation,
        Duration::from_millis(1000 + u64::from(rank.min(30)) * 2000 + backoff),
    );
}
