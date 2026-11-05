//! Device-to-device vault sync over the local network (docs/sync.md).
//!
//! The hub keeps who this device is in its group (main or client), the open
//! links to other devices and whose window has focus. Discovery
//! (`discovery`) finds the devices with the same vault; each connection runs
//! as a `link` task. The merge itself is in `trade_core::sync`.
//!
//! Lock order: the hub's mutex is never held while taking the vault's; a
//! vault save may call into the hub (the sync tap), never the other way.

#[cfg(target_os = "android")]
pub mod background;
mod discovery;
pub mod firewall;
pub mod handover;
mod host;
pub mod keystore;
mod link;
mod pending;
#[cfg(test)]
mod tests;
mod transfer;
#[cfg(test)]
mod transfer_tests;

use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;
use trade_core::ipc_contract::{
    SyncNetworkView, SyncPhase, SyncProgress, SyncRole, SyncSessionView, SyncStatus,
};
use trade_core::service::SyncTap;

use crate::commands::AppState;
use host::Host;

/// Events the windows listen to.
pub const STATUS_EVENT: &str = "sync-status";
pub const OFFER_EVENT: &str = "sync-offer";
pub const PAIRING_EVENT: &str = "sync-pairing";
pub const PLAN_EVENT: &str = "sync-plan-ready";
pub const SYNCED_EVENT: &str = "vault-synced";
pub const ERROR_EVENT: &str = "sync-error";
pub const CLOCK_EVENT: &str = "sync-clock";

/// Failed handshakes or declined pairings from one address before it is
/// ignored for a while.
const MAX_FAILURES: usize = 3;
const FAILURE_WINDOW: Duration = Duration::from_secs(600);

/// What a link task is told to do.
#[derive(Debug, Clone)]
pub(crate) enum Order {
    /// Records saved here (or taken from another device): send them on.
    Changed(Vec<String>),
    /// The user pressed "Pair".
    StartPairing,
    /// "Not now": close quietly, do not offer this device again this run.
    Dismiss,
    /// The user's answer to the code or to the list of changes.
    Decide(bool),
    /// Who should show system notifications (see `protocol::Message::Presence`).
    Presence(bool),
    /// Hand the main role to the other side.
    TakeMain,
    /// The trusted devices changed here (a device was forgotten): send the list.
    SendPeers,
    Close,
}

pub(crate) struct Link {
    pub view: SyncSessionView,
    pub orders: mpsc::UnboundedSender<Order>,
    /// This link is our connection to the main device.
    pub to_main: bool,
    /// The main device dialed this link (the client could not reach it).
    pub callback: bool,
    /// The other side's mDNS instance.
    pub instance: String,
    /// The other side's static Noise key, hex.
    pub remote_key: String,
    /// The other side asked to be the main device.
    pub prefer_main: bool,
    /// A batch was applied on it this session (for "ask").
    pub applied_once: bool,
}

pub(crate) struct Run {
    pub generation: u64,
    pub role: SyncRole,
    pub tag: String,
    pub instance: String,
    /// Main devices of the group seen by discovery: instance → addresses.
    pub mains: BTreeMap<String, Vec<std::net::SocketAddr>>,
    /// The other devices of the group seen by discovery: the main device
    /// calls them back when they cannot reach it.
    pub others: BTreeMap<String, discovery::Seen>,
    /// Connecting to the main device failed: told to it once it calls back.
    pub unreached: bool,
    /// Failed connections to the main device in a row.
    pub dial_failures: u32,
    pub port: u16,
    pub started_ms: i64,
    /// Given by the main device: our place in the queue to take over.
    pub rank: u32,
    pub backup_done: bool,
    /// Devices the user declined or put off: not offered again until sync restarts.
    pub declined: std::collections::BTreeSet<String>,
    /// Instances whose clock is too far off to sync with: not dialed until the
    /// user retries (`reconnect`) or sync restarts.
    pub skewed: std::collections::BTreeSet<String>,
    /// Sessions with the main device that ended before going live, in a row:
    /// each waits longer before connecting again.
    pub retries: u32,
    /// Counts the attempts to dial a main device: a finished attempt only
    /// releases the role set by itself, not by a newer one.
    pub dial_attempt: u64,
    /// Device id → the mDNS instance it showed in a session of this run: how
    /// discovery knows which address belongs to a device that cut this one off.
    pub peer_instances: BTreeMap<String, String>,
    /// Devices the user asked to pair again ("Pair again"): the next dial
    /// to one sends the pairing request at once.
    pub pair_intent: std::collections::BTreeSet<String>,
    pub stop: tokio::sync::watch::Sender<bool>,
    pub announce: Option<discovery::Announcer>,
}

#[derive(Default)]
pub(crate) struct Hub {
    pub run: Option<Run>,
    pub links: BTreeMap<String, Link>,
    pub focused: bool,
    /// Another device could not connect here (see `Hello::unreached`).
    pub inbound_blocked: bool,
    pub last_error: Option<String>,
    /// Errors already announced, per (code, peer device id): the same failure
    /// repeating is shown once, another peer's is still shown.
    pub announced: std::collections::BTreeSet<(String, Option<String>)>,
    /// The last batch applied or change sent (the background lock counts from it).
    pub last_activity: Option<Instant>,
    failures: HashMap<IpAddr, Vec<Instant>>,
    generation: u64,
}

/// Managed state: `app.manage(SyncHub::default())`.
#[derive(Default)]
pub struct SyncHub {
    pub(crate) inner: Mutex<Hub>,
}

pub(crate) fn hub(app: &AppHandle) -> std::sync::MutexGuard<'_, Hub> {
    Host::hub(app)
}

/// Saves of the vault reach the hub through this.
struct Tap(AppHandle);

impl SyncTap for Tap {
    fn changed(&self, keys: &[String]) {
        broadcast(&self.0, keys, None);
    }
}

// -- lifecycle ------------------------------------------------------------------

/// After an unlock: stamp this device's changes as its own and start sync
/// if it is on.
pub fn on_unlock(app: &AppHandle) {
    let device = crate::commands::app_settings::device_id(app);
    {
        let state = app.state::<AppState>();
        if let Ok(mut guard) = state.vault.lock()
            && let Some(vault) = guard.as_mut()
        {
            vault.set_device(&device);
        }
    }
    handover::trust_sender(app);
    restart(app);
}

/// Before a lock: close every link and stop announcing.
pub fn on_lock(app: &AppHandle) {
    stop(app);
}

fn sync_enabled(app: &AppHandle) -> bool {
    crate::commands::app_settings::current_settings(app)
        .sync
        .enabled
}

fn vault_id(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let guard = state.vault.lock().ok()?;
    guard.as_ref().map(|vault| vault.doc.id.clone())
}

/// Stop, then start again if sync is on and a vault is open.
pub fn restart(app: &AppHandle) {
    stop(app);
    clear_errors(&mut Host::hub(app));
    if !sync_enabled(app) {
        emit_status(app);
        return;
    }
    let Some(vault_id) = vault_id(app) else {
        emit_status(app);
        return;
    };
    {
        let state = app.state::<AppState>();
        if let Ok(mut guard) = state.vault.lock()
            && let Some(vault) = guard.as_mut()
        {
            vault.set_sync_tap(Some(Arc::new(Tap(app.clone()))));
        }
    }
    let generation = {
        let mut hub = Host::hub(app);
        hub.generation += 1;
        hub.generation
    };
    #[cfg(target_os = "android")]
    {
        crate::platform_net::multicast(app, true);
        background::arm(app, true);
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = discovery::start(&app, generation, &vault_id).await {
            emit_error(&app, &err, None);
            stop(&app);
        }
        emit_status(&app);
    });
}

pub fn stop(app: &AppHandle) {
    if let Some(run) = teardown(app) {
        #[cfg(target_os = "android")]
        {
            crate::platform_net::multicast(app, false);
            background::arm(app, false);
        }
        let _ = run.stop.send(true);
        if let Some(announce) = run.announce {
            announce.shutdown();
        }
    }
}

/// The part of [`stop`] that does not need the app handle: close the links,
/// take the run, detach from the vault. A start error stays in `last_error`.
pub(crate) fn teardown<H: Host>(app: &H) -> Option<Run> {
    let (run, links) = {
        let mut hub = Host::hub(app);
        hub.inbound_blocked = false;
        let links: Vec<_> = std::mem::take(&mut hub.links).into_values().collect();
        (hub.run.take(), links)
    };
    for link in links {
        let _ = link.orders.send(Order::Close);
    }
    let _ = app.with_vault(|vault| {
        vault.set_sync_tap(None);
        Ok(())
    });
    emit_status(app);
    run
}

/// This device's role in the current run (`None` while sync is off).
pub fn current_role(app: &AppHandle) -> Option<SyncRole> {
    Host::hub(app).run.as_ref().map(|run| run.role)
}

/// Whether some connected device has its window focused.
pub fn any_link_focused(app: &AppHandle) -> bool {
    Host::hub(app).links.values().any(|link| link.view.focused)
}

// -- roles ----------------------------------------------------------------------

/// Whether the background jobs (automations, workflows, imports, alerts,
/// reminders) run here: everywhere but on a client of a main device.
///
/// While searching, they wait too if a usable main device is in sight: it is
/// about to be connected to.
pub fn runs_jobs<H: Host>(app: &H) -> bool {
    let skipped = discovery::skipped_instances(app, false);
    Host::hub(app)
        .run
        .as_ref()
        .is_none_or(|run| match run.role {
            SyncRole::Client => false,
            SyncRole::Searching => run.mains.keys().all(|main| skipped.contains(main)),
            _ => true,
        })
}

/// Whether a system notification should show on this device: here when
/// this window has focus, or when no device of the group has.
pub fn show_system_here(app: &AppHandle) -> bool {
    let hub = Host::hub(app);
    if hub.run.is_none() || hub.focused {
        return true;
    }
    !hub.links.values().any(|link| link.view.focused)
}

/// This window gained or lost focus.
pub fn set_focus(app: &AppHandle, focused: bool) {
    let mut hub = Host::hub(app);
    if hub.focused == focused {
        return;
    }
    hub.focused = focused;
    send_presence(&mut hub);
}

/// Tell each link who else has focus (see `Message::Presence`).
fn send_presence(hub: &mut Hub) {
    let main = hub
        .run
        .as_ref()
        .is_some_and(|run| run.role == SyncRole::Main);
    let here = hub.focused;
    let flags: Vec<(String, bool)> = hub
        .links
        .keys()
        .map(|id| {
            let others = main
                && hub
                    .links
                    .iter()
                    .any(|(other, link)| other != id && link.view.focused);
            (id.clone(), here || others)
        })
        .collect();
    for (id, flag) in flags {
        if let Some(link) = hub.links.get(&id) {
            let _ = link.orders.send(Order::Presence(flag));
        }
    }
}

/// Send keys changed here (or taken from `origin`) to every other link.
pub(crate) fn broadcast<H: Host>(app: &H, keys: &[String], origin: Option<&str>) {
    if keys.is_empty() {
        return;
    }
    let mut hub = Host::hub(app);
    let mut sent = false;
    let mut live = false;
    for (id, link) in &hub.links {
        // Every phase: a link still exchanging or awaiting the user's answer
        // keeps the keys and sends them once it is live.
        if Some(id.as_str()) != origin {
            sent |= link.orders.send(Order::Changed(keys.to_vec())).is_ok();
            live |= link.view.phase == SyncPhase::Live;
        }
    }
    // Only a local edit with a link to send it on postpones the background
    // lock; a batch taken from a peer counts when it is applied.
    if sent && live && origin.is_none() {
        hub.last_activity = Some(Instant::now());
    }
}

pub(crate) fn send<H: Host>(app: &H, session_id: &str, order: Order) -> bool {
    Host::hub(app)
        .links
        .get(session_id)
        .is_some_and(|link| link.orders.send(order).is_ok())
}

/// The trusted devices changed here by a merge from `origin_session`: close
/// the links of devices that are now revoked, tell the others.
pub(crate) fn peers_changed<H: Host>(app: &H, origin_session: &str) {
    let links: Vec<(String, String)> = Host::hub(app)
        .links
        .iter()
        .map(|(id, link)| (id.clone(), link.remote_key.clone()))
        .collect();
    let revoked: Vec<bool> = app
        .with_vault(|vault| {
            Ok(links
                .iter()
                .map(|(_, key)| vault.sync_state().is_revoked(key))
                .collect())
        })
        .unwrap_or_default();
    let hub = Host::hub(app);
    for (index, (id, _)) in links.iter().enumerate() {
        let Some(link) = hub.links.get(id) else {
            continue;
        };
        // A link that is still pairing (or re-pairing a forgotten key) has no
        // trust to lose yet, and no list to receive.
        if matches!(
            link.view.phase,
            SyncPhase::Connecting | SyncPhase::Offer | SyncPhase::Pairing
        ) {
            continue;
        }
        if revoked.get(index).copied().unwrap_or(false) {
            let _ = link.orders.send(Order::Close);
        } else if id != origin_session {
            let _ = link.orders.send(Order::SendPeers);
        }
    }
}

/// Try the devices again now instead of at the next backoff step (a pause
/// was lifted).
pub fn reconnect(app: &AppHandle) {
    discovery::reconnect(app);
}

/// "Pair again" for a device that cut this one off: forget what it told
/// us, remember that the user asked for a pairing, and dial it once with the
/// request. Needs the device to be in sight (discovery saw it in this run).
pub(crate) fn pair_again<H: Host>(app: &H, device_id: &str) -> trade_core::Result<()> {
    let (generation, addrs) = {
        let mut hub = Host::hub(app);
        let not_in_sight =
            || trade_core::Error::NotFound("the other device is not in sight".into());
        let run = hub.run.as_mut().ok_or_else(not_in_sight)?;
        let instance = run.peer_instances.get(device_id).ok_or_else(not_in_sight)?;
        let addrs = run
            .mains
            .get(instance)
            .or_else(|| run.others.get(instance).map(|seen| &seen.addrs))
            .cloned()
            .ok_or_else(not_in_sight)?;
        run.pair_intent.insert(device_id.to_string());
        (run.generation, addrs)
    };
    app.with_vault(|vault| vault.sync_clear_cut(device_id).map(|_| ()))?;
    // Its refusal, if it comes again, is news again.
    peer_recovered(app, device_id);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        discovery::callback(&app, generation, addrs).await;
    });
    Ok(())
}

/// A device was forgotten: close its links, tell everyone else.
pub fn forget_device<H: Host>(app: &H, device_id: &str) {
    let mut hub = Host::hub(app);
    // The main device we dialed is no longer a candidate; pairing with it
    // again is allowed at once.
    let instances: Vec<String> = hub
        .links
        .values()
        .filter(|link| link.view.device_id == device_id)
        .map(|link| link.instance.clone())
        .collect();
    if let Some(run) = hub.run.as_mut() {
        let known = run.peer_instances.get(device_id).cloned();
        for instance in instances.iter().chain(known.iter()) {
            run.mains.remove(instance);
            run.others.remove(instance);
        }
    }
    for link in hub.links.values() {
        let order = if link.view.device_id == device_id {
            Order::Close
        } else {
            Order::SendPeers
        };
        let _ = link.orders.send(order);
    }
}

// -- failures -------------------------------------------------------------------

pub(crate) fn note_failure<H: Host>(app: &H, addr: IpAddr) {
    let mut hub = Host::hub(app);
    let now = Instant::now();
    let list = hub.failures.entry(addr).or_default();
    list.retain(|at| now.duration_since(*at) < FAILURE_WINDOW);
    list.push(now);
}

pub(crate) fn blocked<H: Host>(app: &H, addr: IpAddr) -> bool {
    let mut hub = Host::hub(app);
    let now = Instant::now();
    let Some(list) = hub.failures.get_mut(&addr) else {
        return false;
    };
    list.retain(|at| now.duration_since(*at) < FAILURE_WINDOW);
    list.len() >= MAX_FAILURES
}

// -- status ---------------------------------------------------------------------

pub(crate) fn status<H: Host>(app: &H) -> SyncStatus {
    let settings = app.settings();
    let has_backup = app
        .with_vault(|vault| Ok(vault.has_sync_backup()))
        .unwrap_or(false);
    let key_storage = if settings.sync.enabled {
        app.key_storage()
    } else {
        ""
    };
    let hub = app.hub();
    let network = network_view(&hub);
    SyncStatus {
        enabled: settings.sync.enabled,
        role: hub.run.as_ref().map(|run| run.role).unwrap_or_default(),
        device_id: settings.device_id.clone(),
        device_name: device_name(&settings),
        confirm_first: settings.sync.confirm_first,
        prefer_main: settings.sync.prefer_main,
        key_storage: key_storage.to_string(),
        sessions: hub.links.values().map(|link| link.view.clone()).collect(),
        has_backup,
        background_supported: cfg!(target_os = "android"),
        background_lock_min: settings.sync.background_lock_min(),
        network,
    }
}

fn network_view(hub: &Hub) -> SyncNetworkView {
    let run = hub.run.as_ref();
    let (firewall_fix, firewall_commands) = firewall::fix();
    SyncNetworkView {
        port: run.map(|run| run.port),
        address: run
            .and_then(|_| discovery::local_address())
            .map(|ip| ip.to_string()),
        seen: run.map_or(0, |run| {
            let mut seen: std::collections::BTreeSet<&String> = run.mains.keys().collect();
            seen.extend(run.others.keys());
            u32::try_from(seen.len()).unwrap_or(u32::MAX)
        }),
        since_ms: run.map(|run| run.started_ms),
        inbound_blocked: hub.inbound_blocked,
        firewall_fix,
        firewall_commands,
        last_error: hub.last_error.clone(),
    }
}

/// An exchange with the device completed: its errors may be announced again.
pub(crate) fn peer_recovered<H: Host>(app: &H, device_id: &str) {
    app.hub()
        .announced
        .retain(|(_, peer)| peer.as_deref() != Some(device_id));
}

/// Forget which errors were announced (a new start of sync).
pub(crate) fn clear_errors(hub: &mut Hub) {
    hub.last_error = None;
    hub.announced.clear();
}

pub(crate) fn emit_status<H: Host>(app: &H) {
    app.emit(STATUS_EVENT, status(app));
}

pub(crate) fn emit_error<H: Host>(app: &H, err: &trade_core::Error, peer: Option<(&str, &str)>) {
    emit_error_with(app, err, peer, None);
}

/// [`emit_error`] with the peer's and this app's versions, for a refusal over
/// incompatible versions (`(peer, own)`; the peer's is empty for an old build).
pub(crate) fn emit_error_with<H: Host>(
    app: &H,
    err: &trade_core::Error,
    peer: Option<(&str, &str)>,
    versions: Option<(&str, &str)>,
) {
    let code = err.code().to_string();
    // The same failure of the same peer repeating (a retry loop) is announced
    // once; the same code from another peer is a different failure.
    let repeated = {
        let mut hub = app.hub();
        // A device that cut this one off is not a network fault: the network
        // card keeps its own last error.
        if !matches!(
            code.as_str(),
            "SYNC_PAUSED_BY_PEER" | "SYNC_FORGOTTEN_BY_PEER"
        ) {
            hub.last_error = Some(code.clone());
        }
        !hub.announced
            .insert((code.clone(), peer.map(|(id, _)| id.to_string())))
    };
    if repeated {
        return;
    }
    app.emit(
        ERROR_EVENT,
        trade_core::ipc_contract::SyncErrorEvent {
            code,
            message: err.to_string(),
            peer_name: peer.map(|(_, name)| name.to_string()),
            peer_version: versions.map(|(theirs, _)| theirs.to_string()),
            own_version: versions.map(|(_, own)| own.to_string()),
        },
    );
}

/// The name other devices see: the chosen one, else the host name.
pub fn device_name(settings: &trade_core::appconfig::AppSettings) -> String {
    settings
        .device_name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(host_name)
}

fn host_name() -> String {
    for var in ["HOSTNAME", "COMPUTERNAME"] {
        if let Ok(name) = std::env::var(var)
            && !name.trim().is_empty()
        {
            return name;
        }
    }
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            if cfg!(target_os = "android") {
                "Android".into()
            } else {
                "Computer".into()
            }
        })
}

pub(crate) fn update_link<H: Host>(app: &H, session_id: &str, edit: impl FnOnce(&mut Link)) {
    {
        let mut hub = Host::hub(app);
        if let Some(link) = hub.links.get_mut(session_id) {
            edit(link);
        }
    }
    emit_status(app);
}

pub(crate) fn set_phase<H: Host>(app: &H, session_id: &str, phase: SyncPhase) {
    update_link(app, session_id, |link| link.view.phase = phase);
}

pub(crate) fn set_progress<H: Host>(app: &H, session_id: &str, progress: Option<SyncProgress>) {
    update_link(app, session_id, |link| link.view.progress = progress);
}
