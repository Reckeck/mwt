//! Handing the vault to a new device, the app side (docs/sync.md, "Vault transfer"):
//! the receiving device announces itself by its friendly name
//! for five minutes, the sending one lists such devices, and the sessions in
//! `transfer` do the rest. The received file goes to the app's vault folder;
//! once it is unlocked there, the sending device is trusted for sync.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use trade_core::error::{Error, PeerSyncFailure, Result};
use trade_core::ipc_contract::{
    TransferNearby, TransferPhase, TransferReceiveView, TransferSendInput, TransferSendView,
};
use trade_core::sync::stamp::now_ms;
use trade_core::sync::transfer::{
    CODE_ATTEMPTS, Offer, RECEIVE_WINDOW_SECS, TRANSFER_PROTOCOL, is_code, safe_file_name, sha256,
};

use super::transfer::{Incoming, Me, Outgoing, fingerprint_of, receive, send};
use crate::commands::AppState;

pub const RECEIVE_SERVICE: &str = "_mwt-recv._tcp.local.";
/// The port a receiving device listens on, so a firewall rule can name it.
pub const RECEIVE_PORT: u16 = 53319;
pub const RECEIVE_EVENT: &str = "transfer-receive";
pub const SEND_EVENT: &str = "transfer-send";
pub const NEARBY_EVENT: &str = "transfer-nearby";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

struct Receiving {
    generation: u64,
    view: TransferReceiveView,
    /// "Accept / Decline" for the request on screen.
    decide: Option<mpsc::UnboundedSender<bool>>,
    busy: bool,
    stop: tokio::sync::watch::Sender<bool>,
    daemon: Option<ServiceDaemon>,
}

struct Sending {
    generation: u64,
    view: TransferSendView,
    codes: Option<mpsc::UnboundedSender<String>>,
    task: Option<tauri::async_runtime::JoinHandle<()>>,
}

struct Seen {
    view: TransferNearby,
    addrs: Vec<SocketAddr>,
    fingerprint: String,
}

#[derive(Default)]
struct State {
    generation: u64,
    receiving: Option<Receiving>,
    sending: Option<Sending>,
    browsing: Option<ServiceDaemon>,
    nearby: BTreeMap<String, Seen>,
}

/// Managed state: `app.manage(Handover::default())`.
#[derive(Default)]
pub struct Handover {
    inner: Mutex<State>,
}

fn state(app: &AppHandle) -> MutexGuard<'_, State> {
    let handover = app.state::<Handover>().inner();
    match handover.inner.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn network(err: impl std::fmt::Display) -> Error {
    Error::Network(err.to_string())
}

fn me(app: &AppHandle) -> Me {
    crate::commands::app_settings::ensure_device_name(app);
    let settings = crate::commands::app_settings::current_settings(app);
    Me {
        device_name: super::device_name(&settings),
        device_id: settings.device_id,
    }
}

fn kind() -> &'static str {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        "mobile"
    } else {
        "desktop"
    }
}

fn multicast(app: &AppHandle, on: bool) {
    #[cfg(target_os = "android")]
    crate::platform_net::multicast(app, on);
    #[cfg(not(target_os = "android"))]
    let _ = (app, on);
}

// -- receiving -----------------------------------------------------------------

pub fn receive_view(app: &AppHandle) -> TransferReceiveView {
    state(app)
        .receiving
        .as_ref()
        .map(|receiving| receiving.view.clone())
        .unwrap_or_default()
}

fn emit_receive(app: &AppHandle) {
    let _ = app.emit(RECEIVE_EVENT, receive_view(app));
}

fn update_receive(app: &AppHandle, generation: u64, edit: impl FnOnce(&mut TransferReceiveView)) {
    {
        let mut state = state(app);
        let Some(receiving) = state
            .receiving
            .as_mut()
            .filter(|receiving| receiving.generation == generation)
        else {
            return;
        };
        edit(&mut receiving.view);
    }
    emit_receive(app);
}

async fn listen() -> Result<TcpListener> {
    if let Ok(listener) = TcpListener::bind(("0.0.0.0", RECEIVE_PORT)).await {
        return Ok(listener);
    }
    Ok(TcpListener::bind(("0.0.0.0", 0)).await?)
}

/// "Receive a vault": be visible nearby for five minutes.
pub async fn receive_start(app: &AppHandle) -> Result<TransferReceiveView> {
    if state(app).receiving.is_some() {
        return Ok(receive_view(app));
    }
    let me = me(app);
    let (key, _) = super::keystore::device_key(app)?;
    let listener = listen().await?;
    let port = listener.local_addr()?.port();
    let daemon = ServiceDaemon::new().map_err(network)?;
    let instance = trade_core::sync::session::hex(&rand::random::<[u8; 6]>());
    let fingerprint = fingerprint_of(&key.public_hex());
    let props = [
        ("n", me.device_name.as_str()),
        ("k", kind()),
        ("f", fingerprint.as_str()),
        ("v", "1"),
    ];
    let info = ServiceInfo::new(
        RECEIVE_SERVICE,
        &instance,
        &format!("{instance}.local."),
        "",
        port,
        &props[..],
    )
    .map_err(network)?
    .enable_addr_auto();
    daemon.register(info).map_err(network)?;
    multicast(app, true);

    let expires_ms = i64::try_from(now_ms()).unwrap_or(i64::MAX)
        + i64::try_from(RECEIVE_WINDOW_SECS * 1000).unwrap_or(0);
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let generation = {
        let mut state = state(app);
        state.generation += 1;
        let generation = state.generation;
        state.receiving = Some(Receiving {
            generation,
            view: TransferReceiveView {
                phase: TransferPhase::Waiting,
                device_name: me.device_name.clone(),
                expires_ms: Some(expires_ms),
                ..Default::default()
            },
            decide: None,
            busy: false,
            stop,
            daemon: Some(daemon),
        });
        generation
    };

    // Requests, one at a time.
    let accept_app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                _ = stopped.changed() => break,
                accepted = listener.accept() => {
                    let Ok((stream, addr)) = accepted else { continue };
                    if super::blocked(&accept_app, addr.ip()) {
                        continue;
                    }
                    let free = {
                        let mut state = state(&accept_app);
                        match state.receiving.as_mut().filter(|receiving| receiving.generation == generation) {
                            Some(receiving) if !receiving.busy => {
                                receiving.busy = true;
                                true
                            }
                            _ => false,
                        }
                    };
                    if free {
                        let app = accept_app.clone();
                        let key = key.clone();
                        let me = me.clone();
                        tauri::async_runtime::spawn(async move {
                            serve(&app, generation, stream, addr, &key, &me).await;
                        });
                    }
                }
            }
        }
    });

    // The window closes after five minutes unless a file is on its way.
    let timer_app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(RECEIVE_WINDOW_SECS)).await;
        let expired = state(&timer_app)
            .receiving
            .as_ref()
            .filter(|receiving| receiving.generation == generation)
            .is_some_and(|receiving| {
                !matches!(
                    receiving.view.phase,
                    TransferPhase::Transferring | TransferPhase::Done
                )
            });
        if expired {
            close_receive(&timer_app, generation);
            update_receive(&timer_app, generation, |view| {
                view.phase = TransferPhase::Failed;
                view.error = Some(
                    Error::PeerSync(PeerSyncFailure::TransferExpired)
                        .code()
                        .into(),
                );
            });
        }
    });
    emit_receive(app);
    Ok(receive_view(app))
}

/// Stop being visible; the view stays until [`receive_stop`].
fn close_receive(app: &AppHandle, generation: u64) {
    let daemon = {
        let mut state = state(app);
        let Some(receiving) = state
            .receiving
            .as_mut()
            .filter(|receiving| receiving.generation == generation)
        else {
            return;
        };
        let _ = receiving.stop.send(true);
        receiving.decide = None;
        receiving.daemon.take()
    };
    if let Some(daemon) = daemon {
        let _ = daemon.shutdown();
    }
    multicast(app, false);
}

/// The receiving screen closed.
pub fn receive_stop(app: &AppHandle) {
    let generation = state(app)
        .receiving
        .as_ref()
        .map(|receiving| receiving.generation);
    if let Some(generation) = generation {
        close_receive(app, generation);
    }
    state(app).receiving = None;
    emit_receive(app);
}

/// "Accept" / "Decline".
pub fn receive_decide(app: &AppHandle, accept: bool) -> Result<()> {
    let sender = state(app)
        .receiving
        .as_ref()
        .and_then(|receiving| receiving.decide.clone())
        .ok_or_else(|| Error::NotFound("transfer request".into()))?;
    sender
        .send(accept)
        .map_err(|_| Error::NotFound("transfer request".into()))
}

async fn serve(
    app: &AppHandle,
    generation: u64,
    stream: TcpStream,
    addr: SocketAddr,
    key: &trade_core::sync::session::DeviceKey,
    me: &Me,
) {
    let (decide, mut decisions) = mpsc::unbounded_channel();
    if let Some(receiving) = state(app).receiving.as_mut() {
        receiving.decide = Some(decide);
    }
    let reporter = app.clone();
    let result = receive(stream, key, me, &mut decisions, move |event| {
        update_receive(&reporter, generation, |view| match event {
            Incoming::Request { sender_name } => {
                view.phase = TransferPhase::Request;
                view.sender_name = Some(sender_name);
                view.error = None;
            }
            Incoming::Code(code) => {
                view.phase = TransferPhase::Code;
                view.code = Some(code);
            }
            Incoming::WrongCode { .. } => {
                view.error = Some(Error::PeerSync(PeerSyncFailure::BadCode).code().into());
            }
            Incoming::Progress { done, total } => {
                view.phase = TransferPhase::Transferring;
                view.code = None;
                view.error = None;
                view.done_bytes = done;
                view.total_bytes = total;
            }
        });
    })
    .await;
    match result.and_then(|received| save(app, received)) {
        Ok(path) => {
            close_receive(app, generation);
            update_receive(app, generation, |view| {
                view.phase = TransferPhase::Done;
                view.saved_path = Some(path.display().to_string());
            });
        }
        Err(err) => {
            if matches!(err, Error::PeerSync(PeerSyncFailure::Channel)) {
                super::note_failure(app, addr.ip());
            }
            // Declined, a wrong code three times, a broken connection: the
            // request is gone, this device stays visible until the window ends.
            if let Some(receiving) = state(app)
                .receiving
                .as_mut()
                .filter(|receiving| receiving.generation == generation)
            {
                receiving.busy = false;
                receiving.decide = None;
            }
            update_receive(app, generation, |view| {
                if view.phase != TransferPhase::Failed {
                    view.phase = TransferPhase::Waiting;
                }
                view.sender_name = None;
                view.code = None;
                view.done_bytes = 0;
                view.total_bytes = 0;
                view.error = Some(err.code().into());
            });
        }
    }
}

/// Keep the received vault in the app's vault folder, under its own name
/// (a free one when taken), and remember whom to trust once it is unlocked.
fn save(app: &AppHandle, received: super::transfer::Received) -> Result<PathBuf> {
    let name = safe_file_name(&received.offer.file_name);
    let first = crate::platform_fs::resolve_new_vault(app, &name)?;
    let stem = first
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "vault".into());
    let mut path = first.clone();
    let mut index = 2;
    while path.exists() {
        path = first.with_file_name(format!("{stem} ({index}).wallet"));
        index += 1;
    }
    trade_core::crypto::atomic_write(&path, &received.bytes)?;
    crate::commands::vault_recents::add_recent(app, &path.to_string_lossy());
    // The new device syncs with the one it came from.
    let dir = crate::commands::app_settings::config_dir(app);
    let mut settings = trade_core::appconfig::load_settings(&dir);
    if !settings.sync.enabled {
        settings.sync.enabled = true;
        crate::commands::app_settings::save(&dir, &settings).ok();
    }
    // Written down, not held in memory: the process may be killed before the
    // first unlock (Android).
    let mut settings = trade_core::appconfig::load_settings(&dir);
    super::pending::remember(
        &mut settings,
        &path,
        &received.offer.device_id,
        &received.offer.device_name,
        &received.sender_key,
        now_ms(),
    );
    crate::commands::app_settings::save(&dir, &settings)?;
    Ok(path)
}

/// After an unlock: the vault that just arrived trusts the device it came from.
pub fn trust_sender(app: &AppHandle) {
    let dir = crate::commands::app_settings::config_dir(app);
    let app_state = app.state::<AppState>();
    let Ok(mut guard) = app_state.vault.lock() else {
        return;
    };
    let Some(vault) = guard.as_mut() else {
        return;
    };
    let mut settings = trade_core::appconfig::load_settings(&dir);
    let waiting = settings.pending_trusts.len();
    let mine = super::pending::take(&mut settings, &vault.path, now_ms());
    for pending in &mine {
        super::pending::apply(vault, pending).ok();
    }
    // Entries of other files stay; consumed and expired ones go.
    if settings.pending_trusts.len() != waiting {
        crate::commands::app_settings::save(&dir, &settings).ok();
    }
}

// -- sending -------------------------------------------------------------------

fn nearby_list(state: &State) -> Vec<TransferNearby> {
    state
        .nearby
        .values()
        .map(|seen| seen.view.clone())
        .collect()
}

fn emit_nearby(app: &AppHandle) {
    let list = nearby_list(&state(app));
    let _ = app.emit(NEARBY_EVENT, list);
}

/// "Send to a new device" opened: look for receiving devices.
pub fn browse_start(app: &AppHandle) -> Result<Vec<TransferNearby>> {
    if state(app).browsing.is_some() {
        return Ok(nearby_list(&state(app)));
    }
    let daemon = ServiceDaemon::new().map_err(network)?;
    let events = daemon.browse(RECEIVE_SERVICE).map_err(network)?;
    multicast(app, true);
    {
        let mut state = state(app);
        state.nearby.clear();
        state.browsing = Some(daemon);
    }
    let app = app.clone();
    std::thread::spawn(move || {
        while let Ok(event) = events.recv() {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let text = |name: &str| {
                        info.get_property_val_str(name)
                            .unwrap_or_default()
                            .to_string()
                    };
                    let mut addrs: Vec<SocketAddr> = info
                        .addresses
                        .iter()
                        .map(|ip| SocketAddr::new(ip.to_ip_addr(), info.port))
                        .collect();
                    addrs.sort_by_key(|addr| !addr.is_ipv4());
                    let id = info.fullname.clone();
                    let seen = Seen {
                        view: TransferNearby {
                            id: id.clone(),
                            name: text("n"),
                            kind: text("k"),
                        },
                        addrs,
                        fingerprint: text("f"),
                    };
                    state(&app).nearby.insert(id, seen);
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    state(&app).nearby.remove(&fullname);
                }
                ServiceEvent::SearchStopped(_) => break,
                _ => continue,
            }
            emit_nearby(&app);
        }
    });
    Ok(Vec::new())
}

pub fn browse_stop(app: &AppHandle) {
    let daemon = {
        let mut state = state(app);
        state.nearby.clear();
        state.browsing.take()
    };
    if let Some(daemon) = daemon {
        let _ = daemon.shutdown();
        multicast(app, false);
    }
}

pub fn send_view(app: &AppHandle) -> TransferSendView {
    state(app)
        .sending
        .as_ref()
        .map(|sending| sending.view.clone())
        .unwrap_or_default()
}

fn update_send(app: &AppHandle, generation: u64, edit: impl FnOnce(&mut TransferSendView)) {
    {
        let mut state = state(app);
        let Some(sending) = state
            .sending
            .as_mut()
            .filter(|sending| sending.generation == generation)
        else {
            return;
        };
        edit(&mut sending.view);
    }
    let _ = app.emit(SEND_EVENT, send_view(app));
}

/// "Send": the password is checked here, before anything leaves.
pub fn send_start(app: &AppHandle, input: &TransferSendInput) -> Result<TransferSendView> {
    let (vault, file_name) = {
        let app_state = app.state::<AppState>();
        let mut guard = app_state
            .vault
            .lock()
            .map_err(|_| Error::Crypto("lock poisoned".into()))?;
        let vault = guard.as_mut().ok_or(Error::RequiresUnlock)?;
        vault.check_password(&input.password)?;
        // The file is what travels: the batched changes go into it first.
        vault.flush_pending()?;
        let bytes = std::fs::read(&vault.path).map_err(|err| Error::Io(err.to_string()))?;
        let name = vault
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "vault.wallet".into());
        (bytes, name)
    };
    let (target, addrs, fingerprint) = {
        let state = state(app);
        let seen = state
            .nearby
            .get(&input.id)
            .ok_or_else(|| Error::NotFound("device".into()))?;
        (
            seen.view.clone(),
            seen.addrs.clone(),
            seen.fingerprint.clone(),
        )
    };
    let me = me(app);
    let offer = Offer {
        protocol: TRANSFER_PROTOCOL,
        device_id: me.device_id.clone(),
        device_name: me.device_name.clone(),
        file_name,
        size: vault.len() as u64,
        sha256: sha256(&vault),
    };
    let (key, _) = super::keystore::device_key(app)?;
    let (codes, mut codes_in) = mpsc::unbounded_channel();
    let generation = {
        let mut state = state(app);
        if let Some(task) = state.sending.take().and_then(|sending| sending.task) {
            task.abort();
        }
        state.generation += 1;
        let generation = state.generation;
        state.sending = Some(Sending {
            generation,
            view: TransferSendView {
                phase: TransferPhase::Waiting,
                device_name: target.name.clone(),
                attempts_left: CODE_ATTEMPTS,
                ..Default::default()
            },
            codes: Some(codes),
            task: None,
        });
        generation
    };
    let task_app = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let app = task_app;
        let result = async {
            let mut stream = None;
            for addr in &addrs {
                if let Ok(Ok(connected)) =
                    tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await
                {
                    stream = Some(connected);
                    break;
                }
            }
            let stream = stream.ok_or_else(|| Error::Network("device unreachable".into()))?;
            let reporter = app.clone();
            send(
                stream,
                &key,
                offer,
                &vault,
                &fingerprint,
                &mut codes_in,
                move |event| {
                    update_send(&reporter, generation, |view| match event {
                        Outgoing::Accepted => view.phase = TransferPhase::Code,
                        Outgoing::WrongCode { attempts_left } => {
                            view.attempts_left = attempts_left;
                            view.wrong_code = true;
                        }
                        Outgoing::Progress { done, total } => {
                            view.phase = TransferPhase::Transferring;
                            view.wrong_code = false;
                            view.done_bytes = done;
                            view.total_bytes = total;
                        }
                    });
                },
            )
            .await
        }
        .await;
        match result.and_then(|delivered| record(&app, &delivered)) {
            Ok(()) => update_send(&app, generation, |view| view.phase = TransferPhase::Done),
            Err(err) => update_send(&app, generation, |view| {
                view.phase = TransferPhase::Failed;
                view.error = Some(err.code().into());
            }),
        }
    });
    if let Some(sending) = state(app).sending.as_mut() {
        sending.task = Some(task);
    }
    Ok(send_view(app))
}

/// The vault went: history, trust and a notice every linked device shows.
fn record(app: &AppHandle, delivered: &super::transfer::Delivered) -> Result<()> {
    let ru = crate::scheduler::russian(app);
    let notice = {
        let app_state = app.state::<AppState>();
        let mut guard = app_state
            .vault
            .lock()
            .map_err(|_| Error::Crypto("lock poisoned".into()))?;
        let vault = guard.as_mut().ok_or(Error::RequiresUnlock)?;
        super::pending::record_delivery(vault, delivered, ru)?
    };
    crate::scheduler::deliver(app, vec![notice], true);
    Ok(())
}

/// The code typed on this (sending) device.
pub fn send_code(app: &AppHandle, code: &str) -> Result<()> {
    let code = code.trim();
    if !is_code(code) {
        return Err(Error::PeerSync(PeerSyncFailure::BadCode));
    }
    let generation = {
        let state = state(app);
        let sending = state
            .sending
            .as_ref()
            .ok_or_else(|| Error::NotFound("transfer".into()))?;
        sending
            .codes
            .as_ref()
            .ok_or_else(|| Error::NotFound("transfer".into()))?
            .send(code.to_string())
            .map_err(|_| Error::NotFound("transfer".into()))?;
        sending.generation
    };
    update_send(app, generation, |view| view.wrong_code = false);
    Ok(())
}

/// The sending screen closed (or "Cancel").
pub fn send_cancel(app: &AppHandle) {
    let sending = state(app).sending.take();
    if let Some(task) = sending.and_then(|sending| sending.task) {
        task.abort();
    }
    let _ = app.emit(SEND_EVENT, TransferSendView::default());
}
