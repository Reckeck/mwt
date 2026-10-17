//! Device-to-device vault sync over the local network (docs/sync.md).
//!
//! The hub keeps who this device is in its group (main or client), the open
//! links to other devices and whose window has focus. Discovery
//! (`discovery`) finds the devices with the same vault; each connection runs
//! as a `link` task. The merge itself is in `trade_core::sync`.
//!
//! Lock order: the hub's mutex is never held while taking the vault's; a
//! vault save may call into the hub (the sync tap), never the other way.

mod discovery;
mod host;
pub mod keystore;
mod link;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;
use trade_core::ipc_contract::{SyncPhase, SyncProgress, SyncRole, SyncSessionView, SyncStatus};
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
    /// The user pressed «Сопрячь».
    StartPairing,
    /// «Не сейчас»: close quietly, do not offer this device again this run.
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
    /// The other side asked to be the main device.
    pub prefer_main: bool,
    /// A batch was applied on it this session (for «спрашивать»).
    pub applied_once: bool,
}

pub(crate) struct Run {
    pub generation: u64,
    pub role: SyncRole,
    pub tag: String,
    pub instance: String,
    /// Main devices of the group seen by discovery: instance → addresses.
    pub mains: BTreeMap<String, Vec<std::net::SocketAddr>>,
    /// Given by the main device: our place in the queue to take over.
    pub rank: u32,
    pub next_rank: u32,
    pub backup_done: bool,
    /// Devices the user declined or put off: not offered again until sync restarts.
    pub declined: std::collections::BTreeSet<String>,
    /// Sessions with the main device that ended before going live, in a row:
    /// each waits longer before connecting again.
    pub retries: u32,
    pub stop: tokio::sync::watch::Sender<bool>,
    pub announce: Option<discovery::Announcer>,
}

#[derive(Default)]
pub(crate) struct Hub {
    pub run: Option<Run>,
    pub links: BTreeMap<String, Link>,
    pub focused: bool,
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
    crate::platform_net::multicast(app, true);
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
    let (run, links) = {
        let mut hub = Host::hub(app);
        let links: Vec<_> = std::mem::take(&mut hub.links).into_values().collect();
        (hub.run.take(), links)
    };
    for link in links {
        let _ = link.orders.send(Order::Close);
    }
    if let Some(run) = run {
        #[cfg(target_os = "android")]
        crate::platform_net::multicast(app, false);
        let _ = run.stop.send(true);
        if let Some(announce) = run.announce {
            announce.shutdown();
        }
    }
    let state = app.state::<AppState>();
    if let Ok(mut guard) = state.vault.lock()
        && let Some(vault) = guard.as_mut()
    {
        vault.set_sync_tap(None);
    }
    emit_status(app);
}

// -- roles ----------------------------------------------------------------------

/// Whether the background jobs (automations, workflows, imports, alerts,
/// reminders) run here: everywhere but on a client of a main device.
pub fn runs_jobs(app: &AppHandle) -> bool {
    Host::hub(app)
        .run
        .as_ref()
        .is_none_or(|run| run.role != SyncRole::Client)
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
    let hub = Host::hub(app);
    for (id, link) in &hub.links {
        if Some(id.as_str()) != origin && link.view.phase == SyncPhase::Live {
            let _ = link.orders.send(Order::Changed(keys.to_vec()));
        }
    }
}

pub(crate) fn send<H: Host>(app: &H, session_id: &str, order: Order) -> bool {
    Host::hub(app)
        .links
        .get(session_id)
        .is_some_and(|link| link.orders.send(order).is_ok())
}

/// A device was forgotten: close its links, tell everyone else.
pub fn forget_device(app: &AppHandle, device_id: &str) {
    let hub = Host::hub(app);
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
    }
}

pub(crate) fn emit_status<H: Host>(app: &H) {
    app.emit(STATUS_EVENT, status(app));
}

pub(crate) fn emit_error<H: Host>(app: &H, err: &trade_core::Error, peer: Option<&str>) {
    app.emit(
        ERROR_EVENT,
        trade_core::ipc_contract::SyncErrorEvent {
            code: err.code().to_string(),
            message: err.to_string(),
            peer_name: peer.map(str::to_string),
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
