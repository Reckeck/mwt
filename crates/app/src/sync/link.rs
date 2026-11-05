//! One connection to another device, from the handshake to the end of the
//! session: hello and checks, pairing, the first exchange, then live changes.
//!
//! The vault's mutex is taken only for short, synchronous steps and never
//! held across an `.await` (rule 3).

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio::time::Instant;
use trade_core::error::{Error, PeerSyncFailure, Result};
use trade_core::ipc_contract::{
    SyncClockEvent, SyncConfirmReason, SyncCutReason, SyncOfferEvent, SyncPairingEvent,
    SyncPendingPlan, SyncPhase, SyncPlanEvent, SyncProgress, SyncRole, SyncSessionView,
    VaultSyncedEvent,
};
use trade_core::service::PeerRef;
use trade_core::sync::RecordState;
use trade_core::sync::journal::SyncJournalOutcome;
use trade_core::sync::manifest::Manifest;
use trade_core::sync::protocol::{
    Hello, MANIFEST_PER_MESSAGE, Message, RECORDS_PER_MESSAGE, decode, encode,
};
use trade_core::sync::registry::UNION_DAYS;
use trade_core::sync::session::{Channel, ClockWatch, Handshake, Skew, skew_of};
use trade_core::sync::stamp::{device_tag, now_ms};

use super::host::Host;
use super::{
    CLOCK_EVENT, Link, OFFER_EVENT, Order, PAIRING_EVENT, PLAN_EVENT, SYNCED_EVENT, set_phase,
    set_progress, update_link,
};

/// The largest place in the queue to take over the main role.
pub(crate) const MAX_RANK: u32 = 5;

/// The rank handed to a client at hello time: the number of clients that are
/// live at the main device now (so reconnects do not make it grow).
pub(crate) fn client_rank(hub: &super::Hub) -> u32 {
    let live = hub
        .links
        .values()
        .filter(|link| link.view.phase == SyncPhase::Live)
        .count();
    u32::try_from(live).unwrap_or(MAX_RANK).min(MAX_RANK)
}

/// An older main device counts ranks up for ever: the client caps them.
pub(crate) fn capped_rank(rank: u32) -> u32 {
    rank.min(MAX_RANK)
}

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// A device that forgot the caller waits this long for an explicit pairing
/// request; without one it tells the caller it was forgotten.
#[cfg(not(test))]
const FORGOTTEN_GRACE: Duration = Duration::from_secs(1);
#[cfg(test)]
const FORGOTTEN_GRACE: Duration = Duration::from_millis(300);
const PAIRING_TIMEOUT: Duration = Duration::from_secs(300);
#[cfg(not(test))]
const DECISION_TIMEOUT: Duration = Duration::from_secs(600);
#[cfg(test)]
const DECISION_TIMEOUT: Duration = Duration::from_millis(2500);
#[cfg(not(test))]
const DEBOUNCE: Duration = Duration::from_secs(1);
#[cfg(test)]
const DEBOUNCE: Duration = Duration::from_millis(50);
pub(super) const STABLE_TIME_REPORTS: u32 = 3;
#[cfg(not(test))]
const TIME_EVERY: Duration = Duration::from_secs(30);
#[cfg(test)]
const TIME_EVERY: Duration = Duration::from_millis(150);
#[cfg(not(test))]
const SILENCE_LIMIT: Duration = Duration::from_secs(95);
#[cfg(test)]
const SILENCE_LIMIT: Duration = Duration::from_millis(1000);
/// A peer that sends nothing while the lists are exchanged ends the session
/// (it would otherwise hold its twin out of `hub.links`). A user's question
/// comes after this phase and has its own limit, [`DECISION_TIMEOUT`].
#[cfg(not(test))]
const EXCHANGE_IDLE: Duration = Duration::from_secs(95);
#[cfg(test)]
const EXCHANGE_IDLE: Duration = Duration::from_millis(600);
/// Notices older than this that arrive by sync are logged, not shown.
const FRESH_NOTICE_MS: i64 = 3600 * 1000;

/// How a session ends: quietly (closed, locked, the other side left) or with
/// an error the user is told about.
enum Stop {
    Quiet,
    Failed(Error),
    /// The peer runs an incompatible version: told with its name and versions.
    Incompatible {
        device_id: String,
        name: String,
        version: String,
        own: String,
    },
}

impl From<Error> for Stop {
    fn from(err: Error) -> Self {
        Stop::Failed(err)
    }
}

type Flow<T> = std::result::Result<T, Stop>;

fn failure(kind: PeerSyncFailure) -> Stop {
    Stop::Failed(Error::PeerSync(kind))
}

// -- wire -----------------------------------------------------------------------

pub(super) async fn write_frame(writer: &mut OwnedWriteHalf, bytes: &[u8]) -> Result<()> {
    let len = u16::try_from(bytes.len()).map_err(|_| Error::PeerSync(PeerSyncFailure::Channel))?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(bytes).await?;
    Ok(())
}

pub(super) async fn read_frame(reader: &mut OwnedReadHalf) -> Result<Vec<u8>> {
    let mut head = [0u8; 2];
    reader.read_exact(&mut head).await?;
    let mut body = vec![0u8; usize::from(u16::from_be_bytes(head))];
    reader.read_exact(&mut body).await?;
    Ok(body)
}

/// The encrypted connection. Writing goes through a queue drained by its
/// own task, so a session busy sending never stops reading: two devices
/// sending large batches at each other cannot block one another.
struct Wire {
    channel: Channel,
    out: mpsc::UnboundedSender<Vec<u8>>,
    frames: mpsc::Receiver<Vec<u8>>,
    /// When the last frame came (or the session last showed it was alive).
    heard: Instant,
}

impl Wire {
    async fn send(&mut self, message: &Message) -> Result<()> {
        let bytes = encode(message)?;
        for frame in self.channel.seal(&bytes)? {
            self.out
                .send(frame)
                .map_err(|_| Error::PeerSync(PeerSyncFailure::Channel))?;
        }
        Ok(())
    }

    /// The next whole message; `None` when the connection closed.
    async fn next(&mut self) -> Flow<Option<Message>> {
        loop {
            let Some(frame) = self.frames.recv().await else {
                return Ok(None);
            };
            self.heard = Instant::now();
            if let Some(bytes) = self.channel.open(&frame)? {
                return Ok(Some(decode(&bytes)?));
            }
        }
    }
}

// -- entry ----------------------------------------------------------------------

/// `dialer`: this device opened the connection. Which side is the main
/// device comes from the hellos: a client dials the main device, or the main
/// device calls back a client that cannot reach it. True when a verified
/// link came up (the hellos passed and the link was entered in the hub).
pub(crate) async fn run<H: Host>(
    app: H,
    stream: TcpStream,
    addr: SocketAddr,
    generation: u64,
    dialer: bool,
) -> bool {
    let session_id = uuid::Uuid::new_v4().to_string();
    let mut to_main = false;
    let mut up = false;
    let result = session(
        &app,
        stream,
        addr,
        generation,
        dialer,
        &session_id,
        &mut to_main,
        &mut up,
    )
    .await;
    let peer = forget_link(&app, &session_id);
    if let Err(Stop::Failed(err)) = &result {
        if matches!(
            err,
            Error::PeerSync(PeerSyncFailure::OtherVault | PeerSyncFailure::Channel)
        ) {
            super::note_failure(&app, addr.ip());
        }
        super::emit_error(
            &app,
            err,
            peer.as_ref().map(|(id, name)| (id.as_str(), name.as_str())),
        );
    }
    if let Err(Stop::Incompatible {
        device_id,
        name,
        version,
        own,
    }) = &result
    {
        super::emit_error_with(
            &app,
            &Error::PeerSync(PeerSyncFailure::VersionMismatch),
            Some((device_id.as_str(), name.as_str())),
            Some((version.as_str(), own.as_str())),
        );
    }
    if to_main {
        app.main_lost(generation);
    }
    super::emit_status(&app);
    up
}

/// Drop the link from the hub; the device id and name it showed.
pub(super) fn forget_link<H: Host>(app: &H, session_id: &str) -> Option<(String, String)> {
    let mut hub = app.hub();
    let gone = hub
        .links
        .remove(session_id)
        .map(|link| (link.view.device_id, link.view.name));
    // The closed link may have been the focused one: tell the rest.
    super::send_presence(&mut hub);
    gone
}

pub(super) struct Peer {
    pub(super) session_id: String,
    pub(super) device_id: String,
    pub(super) name: String,
    pub(super) remote_key: String,
    /// The vault this session was opened for (taken at the hello): a vault
    /// switched under a running session must not receive its records.
    pub(super) vault_id: String,
    /// The other side handed the main role over while this session was not
    /// live yet (pairing, exchange, a question): done when `live` starts.
    pub(super) take_main_heard: AtomicBool,
    /// Same for the order to hand it over (`hand_over_if_preferred`).
    pub(super) take_main_ordered: AtomicBool,
    /// Consecutive `Time` reports within the warn bound since the last drift.
    pub(super) stable_reports: AtomicU32,
}

/// Runs `work` on the open vault only while it is still the vault this
/// session was opened for; a vault switched under the session ends it quietly.
fn with_peer_vault<H: Host, T>(
    app: &H,
    peer: &Peer,
    work: impl FnOnce(&mut trade_core::service::UnlockedVault) -> Result<T>,
) -> Flow<T> {
    let outcome = app.with_vault(|vault| {
        if vault.doc.id != peer.vault_id {
            return Ok(None);
        }
        work(vault).map(Some)
    })?;
    outcome.ok_or(Stop::Quiet)
}

/// Merge the other side's trusted devices and tell the other links; whether
/// the other side's own key is revoked now. Refuses (quietly) when the open
/// vault is no longer the session's.
fn take_peers<H: Host>(
    app: &H,
    peer: &Peer,
    peers: &[trade_core::sync::TrustedPeer],
) -> Flow<bool> {
    let taken = app.with_vault(|vault| {
        if vault.doc.id != peer.vault_id {
            return Ok(None);
        }
        let changed = vault.sync_merge_peers(peers)?;
        Ok(Some((
            changed,
            vault.sync_state().is_revoked(&peer.remote_key),
        )))
    })?;
    let Some((changed, revoked)) = taken else {
        return Err(Stop::Quiet);
    };
    if changed {
        super::peers_changed(app, &peer.session_id);
    }
    Ok(revoked)
}

#[allow(clippy::too_many_arguments)]
async fn session<H: Host>(
    app: &H,
    stream: TcpStream,
    addr: SocketAddr,
    generation: u64,
    dialer: bool,
    session_id: &str,
    to_main_out: &mut bool,
    up: &mut bool,
) -> Flow<()> {
    let key = app.device_key()?;
    stream.set_nodelay(true).ok();
    let (mut reader, mut writer) = stream.into_split();

    let channel = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
        let mut handshake = if dialer {
            Handshake::initiator(&key)?
        } else {
            Handshake::responder(&key)?
        };
        while !handshake.is_finished() {
            if handshake.my_turn() {
                write_frame(&mut writer, &handshake.write()?).await?;
            } else {
                handshake.read(&read_frame(&mut reader).await?)?;
            }
        }
        handshake.finish()
    })
    .await
    .map_err(|_| failure(PeerSyncFailure::Channel))??;

    let (frames_in, frames) = mpsc::channel::<Vec<u8>>(64);
    tauri::async_runtime::spawn(async move {
        while let Ok(frame) = read_frame(&mut reader).await {
            if frames_in.send(frame).await.is_err() {
                break;
            }
        }
    });
    let (out, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
    tauri::async_runtime::spawn(async move {
        while let Some(frame) = outgoing.recv().await {
            if write_frame(&mut writer, &frame).await.is_err() {
                break;
            }
        }
    });
    let mut wire = Wire {
        channel,
        out,
        frames,
        heard: Instant::now(),
    };
    let remote_key = wire.channel.remote_hex();

    // -- hello --
    // A forgotten (revoked) key is not trusted, so it asks for pairing: it only
    // returns through a pairing code confirmed on both devices.
    let (vault_id, trusted) = app.with_vault(|vault| {
        Ok((
            vault.doc.id.clone(),
            vault.sync_state().trusted(&remote_key).is_some(),
        ))
    })?;
    let settings = app.settings();
    let (main_here, rank, instance, unreached) = {
        let hub = app.hub();
        match hub.run.as_ref().filter(|run| run.generation == generation) {
            Some(run) if run.role == SyncRole::Main => {
                (true, client_rank(&hub), run.instance.clone(), false)
            }
            // A dial that got through has not failed to reach anyone.
            Some(run) => (false, 0, run.instance.clone(), run.unreached && !dialer),
            None => return Err(Stop::Quiet),
        }
    };
    let versions = app.versions();
    wire.send(&Message::Hello(Hello {
        protocol: versions.0,
        format: versions.1,
        device_id: settings.device_id.clone(),
        device_name: super::device_name(&settings),
        proof: wire.channel.proof(&vault_id),
        now_ms: now_ms(),
        main: main_here,
        prefer_main: settings.sync.prefer_main,
        needs_pairing: !trusted,
        rank,
        instance,
        unreached,
        app_version: app.app_version(),
    }))
    .await?;
    let theirs = match tokio::time::timeout(HANDSHAKE_TIMEOUT, wire.next())
        .await
        .map_err(|_| failure(PeerSyncFailure::Channel))??
    {
        Some(Message::Hello(hello)) => hello,
        Some(_) => return Err(failure(PeerSyncFailure::Channel)),
        None => return Err(Stop::Quiet),
    };
    if !wire.channel.verify_proof(&vault_id, &theirs.proof) {
        return Err(failure(PeerSyncFailure::OtherVault));
    }
    if (theirs.protocol, theirs.format) != versions {
        return Err(Stop::Incompatible {
            device_id: theirs.device_id.clone(),
            name: theirs.device_name.clone(),
            version: theirs.app_version.clone(),
            own: app.app_version(),
        });
    }
    // Which address is this device: discovery needs it to skip a device that
    // cut this one off (or that was paused here), even when the session ends
    // right below.
    if !theirs.instance.is_empty()
        && let Some(run) = app.hub().run.as_mut()
    {
        run.peer_instances
            .insert(theirs.device_id.clone(), theirs.instance.clone());
    }
    let declined = app
        .hub()
        .run
        .as_ref()
        .is_some_and(|run| run.declined.contains(&theirs.device_id));
    // Paused by the user (a declined list of changes): close without a word.
    let paused = app.with_vault(|vault| Ok(vault.sync_is_paused(&theirs.device_id)))?;
    if theirs.device_id == settings.device_id || declined {
        return Err(Stop::Quiet);
    }
    if paused {
        // Told, not left to knock: the other side stops calling and says why.
        let _ = wire
            .send(&Message::Bye {
                reason: SyncCutReason::Paused.wire().into(),
            })
            .await;
        return Err(Stop::Quiet);
    }
    // Exactly one side of a link is the main device; two main devices sort
    // it out through discovery.
    if main_here == theirs.main {
        return Err(Stop::Quiet);
    }
    // A forgotten key only returns through a pairing somebody asked for: a
    // pairing request right after the hellos. Anything else is told so.
    let mut asked_to_pair = false;
    if app.with_vault(|vault| Ok(vault.sync_state().is_revoked(&remote_key)))? {
        match tokio::time::timeout(FORGOTTEN_GRACE, wire.next()).await {
            Ok(Ok(Some(Message::PairRequest))) => asked_to_pair = true,
            Ok(Ok(Some(Message::Bye { .. }) | None)) => return Err(Stop::Quiet),
            Ok(Err(stop)) => return Err(stop),
            Ok(Ok(Some(_))) | Err(_) => {
                let _ = wire
                    .send(&Message::Bye {
                        reason: SyncCutReason::Forgotten.wire().into(),
                    })
                    .await;
                return Err(Stop::Quiet);
            }
        }
    }
    let to_main = theirs.main;
    *to_main_out = to_main;
    let callback = dialer == main_here;
    let mut watch = ClockWatch::new(theirs.now_ms, now_ms());
    match skew_of(watch.offset()) {
        Skew::Refuse(_) => {
            // Redialing cannot help: skip this device until the user retries.
            if !theirs.instance.is_empty()
                && let Some(run) = app.hub().run.as_mut()
            {
                run.skewed.insert(theirs.instance.clone());
            }
            return Err(failure(PeerSyncFailure::ClockSkew));
        }
        Skew::Warn(_) => {
            app.emit(
                CLOCK_EVENT,
                SyncClockEvent {
                    session_id: session_id.to_string(),
                    peer_name: theirs.device_name.clone(),
                    offset_ms: watch.offset(),
                    unreliable: false,
                },
            );
        }
        Skew::Fine => {}
    }

    let (orders_in, mut orders) = mpsc::unbounded_channel();
    let intent;
    {
        let mut guard = app.hub();
        let hub = &mut *guard;
        let Some(run) = hub.run.as_mut().filter(|run| run.generation == generation) else {
            return Err(Stop::Quiet);
        };
        // The user asked to pair this device again: the request goes out first.
        intent = dialer && run.pair_intent.remove(&theirs.device_id);
        if to_main {
            // A client called back while it was still looking.
            if run.role == SyncRole::Main {
                return Err(Stop::Quiet);
            }
            run.role = SyncRole::Client;
            run.rank = capped_rank(theirs.rank);
        }
        // Both connections came up at once: keep the one the client dialed.
        let twins: Vec<(String, bool)> = hub
            .links
            .iter()
            .filter(|(_, link)| link.view.device_id == theirs.device_id)
            .map(|(id, link)| (id.clone(), link.callback))
            .collect();
        if !twins.is_empty() {
            if callback || twins.iter().any(|(_, other)| !other) {
                return Err(Stop::Quiet);
            }
            for (id, _) in &twins {
                if let Some(link) = hub.links.get(id) {
                    let _ = link.orders.send(Order::Close);
                }
            }
        }
        if main_here {
            if theirs.unreached {
                hub.inbound_blocked = true;
            } else if !dialer {
                hub.inbound_blocked = false;
            }
        }
        if dialer {
            // The other device answered: the dial worked.
            run.unreached = false;
            run.dial_failures = 0;
        }
        hub.links.insert(
            session_id.to_string(),
            Link {
                view: SyncSessionView {
                    session_id: session_id.to_string(),
                    device_id: theirs.device_id.clone(),
                    name: theirs.device_name.clone(),
                    phase: SyncPhase::Connecting,
                    unreliable: false,
                    clock_offset_ms: watch.offset(),
                    progress: None,
                    focused: false,
                    pairing_code: None,
                    pairing_expires_ms: None,
                    plan: None,
                },
                orders: orders_in,
                to_main,
                callback,
                instance: theirs.instance.clone(),
                remote_key: remote_key.clone(),
                prefer_main: theirs.prefer_main,
                applied_once: false,
            },
        );
    }
    *up = true;
    super::emit_status(app);
    let peer = Peer {
        session_id: session_id.to_string(),
        device_id: theirs.device_id.clone(),
        name: theirs.device_name.clone(),
        remote_key,
        vault_id: vault_id.clone(),
        take_main_heard: AtomicBool::new(false),
        take_main_ordered: AtomicBool::new(false),
        stable_reports: AtomicU32::new(0),
    };

    let mut reason = None;
    if !trusted || theirs.needs_pairing {
        pair(
            app,
            &mut wire,
            &mut orders,
            &peer,
            addr,
            intent,
            asked_to_pair,
        )
        .await?;
        reason = Some(SyncConfirmReason::Paired);
    }
    let _ = addr;
    let mut pending = BTreeSet::new();
    exchange(
        app,
        &mut wire,
        &mut orders,
        &peer,
        &mut watch,
        reason,
        &mut pending,
    )
    .await?;
    live(app, &mut wire, &mut orders, &peer, &mut watch, pending).await
}

// -- pairing --------------------------------------------------------------------

async fn pair<H: Host>(
    app: &H,
    wire: &mut Wire,
    orders: &mut mpsc::UnboundedReceiver<Order>,
    peer: &Peer,
    addr: SocketAddr,
    ask_now: bool,
    asked: bool,
) -> Flow<()> {
    set_phase(app, &peer.session_id, SyncPhase::Offer);
    app.emit(
        OFFER_EVENT,
        SyncOfferEvent {
            session_id: peer.session_id.clone(),
            peer_name: peer.name.clone(),
        },
    );
    let code = wire.channel.pairing_code();
    let expires_ms =
        now_ms().saturating_add(u64::try_from(PAIRING_TIMEOUT.as_millis()).unwrap_or(u64::MAX));
    let show_code = |app: &H| {
        update_link(app, &peer.session_id, |link| {
            link.view.phase = SyncPhase::Pairing;
            link.view.pairing_code = Some(code.clone());
            link.view.pairing_expires_ms = Some(expires_ms);
        });
        app.emit(
            PAIRING_EVENT,
            SyncPairingEvent {
                session_id: peer.session_id.clone(),
                peer_name: peer.name.clone(),
                code: code.clone(),
            },
        );
    };
    let deadline = tokio::time::sleep(PAIRING_TIMEOUT);
    tokio::pin!(deadline);
    let (mut shown, mut mine, mut theirs) = (false, false, false);
    // The other side's list of devices is not merged from a device nobody has
    // confirmed yet (that would let it add trusted devices): kept, and taken
    // in once the pairing is through.
    let mut held_peers: Option<Vec<trade_core::sync::TrustedPeer>> = None;
    // The pairing was asked for before this point: the user's ("Pair
    // again") or the other side's request that opened the session.
    if ask_now || asked {
        shown = true;
        show_code(app);
    }
    if ask_now {
        wire.send(&Message::PairRequest).await?;
    }
    loop {
        tokio::select! {
            () = &mut deadline => {
                decline(app, peer);
                return Err(Stop::Quiet);
            }
            order = orders.recv() => match order {
                Some(Order::Dismiss) => {
                    decline(app, peer);
                    return Err(Stop::Quiet);
                }
                Some(Order::StartPairing) => {
                    if !shown {
                        shown = true;
                        show_code(app);
                    }
                    wire.send(&Message::PairRequest).await?;
                }
                Some(Order::Decide(accept)) if shown => {
                    wire.send(&Message::Pairing { accept }).await?;
                    if !accept {
                        decline(app, peer);
                        // Our own decision is no error to show, but repeated asks
                        // from this address stay throttled.
                        super::note_failure(app, addr.ip());
                        return Err(Stop::Quiet);
                    }
                    mine = true;
                }
                Some(Order::TakeMain) => peer.take_main_ordered.store(true, Ordering::SeqCst),
                // The exchange that follows the pairing sends our list anyway,
                // and nothing is told to a device that is not confirmed.
                Some(Order::Close) | None => return Err(Stop::Quiet),
                Some(_) => {}
            },
            message = wire.next() => match message? {
                Some(Message::Peers { peers }) => held_peers = Some(peers),
                Some(Message::TakeMain) => peer.take_main_heard.store(true, Ordering::SeqCst),
                Some(Message::PairRequest) => {
                    if !shown {
                        shown = true;
                        show_code(app);
                    }
                }
                Some(Message::Pairing { accept }) => {
                    if !accept {
                        decline(app, peer);
                        return Err(failure(PeerSyncFailure::Rejected));
                    }
                    theirs = true;
                }
                Some(Message::Bye { reason }) => return Err(heard_bye(app, peer, &reason)),
                None => return Err(Stop::Quiet),
                Some(_) => {}
            },
        }
        if mine && theirs {
            update_link(app, &peer.session_id, |link| {
                link.view.pairing_code = None;
                link.view.pairing_expires_ms = None;
            });
            with_peer_vault(app, peer, |vault| {
                vault.sync_trust_peer(&peer.device_id, &peer.name, &peer.remote_key)
            })?;
            if let Some(peers) = held_peers.take()
                && take_peers(app, peer, &peers)?
            {
                return Err(failure(PeerSyncFailure::Revoked));
            }
            return Ok(());
        }
    }
}

/// The other device said goodbye with a reason. `paused` and `forgotten` mean
/// it cut this device off on purpose: remember it (so discovery stops dialing
/// it) and tell the user once. Any other reason is a plain quiet close, as
/// before (older builds send none of these).
fn heard_bye<H: Host>(app: &H, peer: &Peer, reason: &str) -> Stop {
    let Some(cut) = SyncCutReason::from_wire(reason) else {
        return Stop::Quiet;
    };
    let news = with_peer_vault(app, peer, |vault| vault.sync_cut_by(&peer.device_id, cut))
        .unwrap_or(false);
    if news {
        let failure = match cut {
            SyncCutReason::Paused => PeerSyncFailure::PausedByPeer,
            SyncCutReason::Forgotten => PeerSyncFailure::ForgottenByPeer,
        };
        super::emit_error(
            app,
            &Error::PeerSync(failure),
            Some((peer.device_id.as_str(), peer.name.as_str())),
        );
    }
    Stop::Quiet
}

/// Do not offer this device again until sync restarts.
fn decline<H: Host>(app: &H, peer: &Peer) {
    if let Some(run) = app.hub().run.as_mut() {
        run.declined.insert(peer.device_id.clone());
    }
}

// -- first exchange -------------------------------------------------------------

pub(super) fn check_stamps<H: Host>(
    app: &H,
    peer: &Peer,
    watch: &ClockWatch,
    records: &[(String, RecordState)],
) {
    let now = now_ms();
    let own_tag = device_tag(&peer.device_id);
    if records
        .iter()
        .filter_map(|(_, state)| state.latest())
        // Only what the peer wrote itself: a stamp it relays says nothing
        // about its own clock.
        .filter(|stamp| stamp.device == own_tag)
        .any(|stamp| watch.check_stamp(stamp.wall, now).is_some())
    {
        mark_unreliable(app, peer, watch);
    }
}

/// A `Time` message from the peer, seen at `local` (ms).
pub(super) fn time_report<H: Host>(
    app: &H,
    peer: &Peer,
    watch: &mut ClockWatch,
    remote: u64,
    local: u64,
) {
    if watch.observe(remote, local).is_some() {
        mark_unreliable(app, peer, watch);
        return;
    }
    let stable = peer.stable_reports.fetch_add(1, Ordering::SeqCst) + 1;
    if stable >= STABLE_TIME_REPORTS {
        clear_unreliable(app, peer, watch);
    }
}

fn clear_unreliable<H: Host>(app: &H, peer: &Peer, watch: &ClockWatch) {
    let marked = app
        .hub()
        .links
        .get(&peer.session_id)
        .is_some_and(|link| link.view.unreliable);
    if !marked {
        return;
    }
    update_link(app, &peer.session_id, |link| link.view.unreliable = false);
    app.emit(
        CLOCK_EVENT,
        SyncClockEvent {
            session_id: peer.session_id.clone(),
            peer_name: peer.name.clone(),
            offset_ms: watch.offset(),
            unreliable: false,
        },
    );
}

fn mark_unreliable<H: Host>(app: &H, peer: &Peer, watch: &ClockWatch) {
    peer.stable_reports.store(0, Ordering::SeqCst);
    let already = app
        .hub()
        .links
        .get(&peer.session_id)
        .is_some_and(|link| link.view.unreliable);
    if already {
        return;
    }
    update_link(app, &peer.session_id, |link| link.view.unreliable = true);
    app.emit(
        CLOCK_EVENT,
        SyncClockEvent {
            session_id: peer.session_id.clone(),
            peer_name: peer.name.clone(),
            offset_ms: watch.offset(),
            unreliable: true,
        },
    );
}

async fn send_records<H: Host>(app: &H, peer: &Peer, wire: &mut Wire, keys: &[String]) -> Flow<()> {
    if keys.is_empty() {
        wire.send(&Message::Records {
            records: Vec::new(),
            last: true,
        })
        .await?;
        return Ok(());
    }
    let chunks: Vec<&[String]> = keys.chunks(RECORDS_PER_MESSAGE).collect();
    let count = chunks.len();
    for (index, chunk) in chunks.into_iter().enumerate() {
        let records = with_peer_vault(app, peer, |vault| vault.sync_records(chunk))?;
        wire.send(&Message::Records {
            records: records.into_iter().collect(),
            last: index + 1 == count,
        })
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn exchange<H: Host>(
    app: &H,
    wire: &mut Wire,
    orders: &mut mpsc::UnboundedReceiver<Order>,
    peer: &Peer,
    watch: &mut ClockWatch,
    reason: Option<SyncConfirmReason>,
    pending: &mut BTreeSet<String>,
) -> Flow<()> {
    set_phase(app, &peer.session_id, SyncPhase::Exchanging);
    let peers = with_peer_vault(app, peer, |vault| vault.sync_peers_to_send())?;
    wire.send(&Message::Peers { peers }).await?;
    let since = now_ms().saturating_sub(UNION_DAYS * 24 * 3600 * 1000);
    let (mine, mark) = with_peer_vault(app, peer, |vault| vault.sync_manifest_marked(Some(since)))?;
    let entries: Vec<(String, u64)> = mine
        .iter()
        .map(|(key, digest)| (key.clone(), *digest))
        .collect();
    let parts: Vec<&[(String, u64)]> = if entries.is_empty() {
        vec![&[]]
    } else {
        entries.chunks(MANIFEST_PER_MESSAGE).collect()
    };
    let count = parts.len();
    for (index, part) in parts.into_iter().enumerate() {
        wire.send(&Message::Manifest {
            entries: part.to_vec(),
            last: index + 1 == count,
        })
        .await?;
    }

    let mut theirs = Manifest::new();
    let mut incoming: BTreeMap<String, RecordState> = BTreeMap::new();
    let (mut asked, mut answered, mut received) = (false, false, false);
    let mut total = 0u32;
    // Only the wait for the peer is limited; the user's question comes after
    // this loop (in `apply`) with its own limit.
    let mut idle_at = Instant::now() + EXCHANGE_IDLE;
    while !(asked && answered && received) {
        tokio::select! {
            () = tokio::time::sleep_until(idle_at) => {
                let _ = wire.send(&Message::Bye { reason: "timeout".into() }).await;
                return Err(Stop::Quiet);
            }
            order = orders.recv() => match order {
                Some(Order::Changed(keys)) => pending.extend(keys),
                Some(Order::Presence(focused)) => wire.send(&Message::Presence { focused }).await?,
                Some(Order::TakeMain) => peer.take_main_ordered.store(true, Ordering::SeqCst),
                Some(Order::Close) | None => return Err(Stop::Quiet),
                // `live` starts with a fresh list.
                Some(_) => {}
            },
            message = wire.next() => { idle_at = Instant::now() + EXCHANGE_IDLE; match message? {
                Some(Message::Peers { peers }) => {
                    if take_peers(app, peer, &peers)? {
                        return Err(failure(PeerSyncFailure::Revoked));
                    }
                }
                Some(Message::TakeMain) => peer.take_main_heard.store(true, Ordering::SeqCst),
                Some(Message::Manifest { entries, last }) => {
                    theirs.extend(entries);
                    if last {
                        let (want, _) = app.with_vault(|vault| Ok(vault.sync_diff(&mine, &theirs)))?;
                        total = u32::try_from(want.len()).unwrap_or(u32::MAX);
                        if total > 0 {
                            set_progress(app, &peer.session_id, Some(SyncProgress { received: 0, total }));
                        }
                        wire.send(&Message::Request { keys: want }).await?;
                        asked = true;
                    }
                }
                Some(Message::Request { keys }) => {
                    send_records(app, peer, wire, &keys).await?;
                    answered = true;
                }
                Some(Message::Records { records, last }) => {
                    check_stamps(app, peer, watch, &records);
                    incoming.extend(records);
                    if total > 0 {
                        let got = u32::try_from(incoming.len()).unwrap_or(u32::MAX);
                        set_progress(app, &peer.session_id, Some(SyncProgress { received: got, total }));
                    }
                    if last {
                        received = true;
                    }
                }
                Some(Message::Time { now_ms: remote }) => {
                    time_report(app, peer, watch, remote, now_ms());
                }
                Some(Message::Presence { focused }) => presence(app, peer, focused),
                Some(Message::Bye { reason }) => return Err(heard_bye(app, peer, &reason)),
                None => return Err(Stop::Quiet),
                Some(_) => {}
            } },
        }
    }
    set_progress(app, &peer.session_id, None);
    apply(app, wire, orders, peer, watch, incoming, reason, pending).await?;
    with_peer_vault(app, peer, |vault| {
        vault.sync_mark_synced(&peer.device_id, Some(&mark))?;
        // Only an applied exchange vouches for our deletions: its manifest
        // says which of them the peer holds.
        vault.sync_ack_tombstones(&peer.device_id, &theirs)?;
        // It talks to us again: whatever it told us before is over.
        vault.sync_clear_cut(&peer.device_id).map(|_| ())
    })?;
    super::peer_recovered(app, &peer.device_id);
    Ok(())
}

fn presence<H: Host>(app: &H, peer: &Peer, focused: bool) {
    update_link(app, &peer.session_id, |link| link.view.focused = focused);
    let mut hub = app.hub();
    if hub
        .run
        .as_ref()
        .is_some_and(|run| run.role == SyncRole::Main)
    {
        super::send_presence(&mut hub);
    }
}

/// Apply records from the other device, asking first when needed.
///
/// Records that arrive while the question is open are held, never applied
/// with the answer: the answer covers the list the user saw. They go
/// through a new list and a new question of their own (and, when they
/// change nothing, are dropped). Held records are keyed by record, so they
/// stay bounded by the vault; an unanswered question ends the session
/// after [`DECISION_TIMEOUT`] and the next session sends them again.
#[allow(clippy::too_many_arguments)]
async fn apply<H: Host>(
    app: &H,
    wire: &mut Wire,
    orders: &mut mpsc::UnboundedReceiver<Order>,
    peer: &Peer,
    watch: &mut ClockWatch,
    incoming: BTreeMap<String, RecordState>,
    reason: Option<SyncConfirmReason>,
    pending: &mut BTreeSet<String>,
) -> Flow<()> {
    let mut next = incoming;
    let mut reason = reason;
    let mut first = true;
    loop {
        let mut held = BTreeMap::new();
        let mut asked = None;
        apply_shown(
            app, wire, orders, peer, watch, next, reason, pending, &mut held, &mut asked, first,
        )
        .await?;
        if held.is_empty() {
            return Ok(());
        }
        // Asked once, so asked again: the same reason, a fresh list.
        next = held;
        reason = asked;
        first = false;
    }
}

/// One round of [`apply`]: `incoming` is exactly what the question shows.
/// `held` takes what arrives during the question; `asked` the reason it was
/// asked for. A round that is not the `first` one applies nothing when its
/// list is empty.
#[allow(clippy::too_many_arguments)]
async fn apply_shown<H: Host>(
    app: &H,
    wire: &mut Wire,
    orders: &mut mpsc::UnboundedReceiver<Order>,
    peer: &Peer,
    watch: &mut ClockWatch,
    incoming: BTreeMap<String, RecordState>,
    reason: Option<SyncConfirmReason>,
    pending: &mut BTreeSet<String>,
    held: &mut BTreeMap<String, RecordState>,
    asked: &mut Option<SyncConfirmReason>,
    first: bool,
) -> Flow<()> {
    if incoming.is_empty() {
        return Ok(());
    }
    let (unreliable, applied_once) = app
        .hub()
        .links
        .get(&peer.session_id)
        .map(|link| (link.view.unreliable, link.applied_once))
        .unwrap_or_default();
    let confirm_first = app.settings().sync.confirm_first;
    let reason = reason.or(if unreliable {
        Some(SyncConfirmReason::Unreliable)
    } else if confirm_first && !applied_once {
        Some(SyncConfirmReason::First)
    } else {
        None
    });
    if let Some(reason) = reason {
        let summary =
            app.with_vault(|vault| vault.sync_preview_from(&incoming, &peer.device_id))?;
        if summary.is_empty() && !first {
            return Ok(());
        }
        if !summary.is_empty() {
            // Capital and balances afterwards; the question still shows
            // without them when they cannot be worked out.
            let after = app
                .with_vault(|vault| vault.sync_preview_after(&incoming))
                .unwrap_or_default();
            *asked = Some(reason);
            update_link(app, &peer.session_id, |link| {
                link.view.phase = SyncPhase::AwaitingConfirm;
                link.view.plan = Some(SyncPendingPlan {
                    reason,
                    summary: summary.clone(),
                    after: after.clone(),
                });
            });
            app.emit(
                PLAN_EVENT,
                SyncPlanEvent {
                    session_id: peer.session_id.clone(),
                    peer_name: peer.name.clone(),
                    reason,
                    summary: summary.clone(),
                    after: after.clone(),
                },
            );
            let deadline = tokio::time::sleep(DECISION_TIMEOUT);
            tokio::pin!(deadline);
            // The other side closes a silent session: keep telling it the time.
            let mut ticker = tokio::time::interval(TIME_EVERY);
            // `None`: no answer in time. That is not a decline.
            let decision = loop {
                tokio::select! {
                    () = &mut deadline => break None,
                    _ = ticker.tick() => wire.send(&Message::Time { now_ms: now_ms() }).await?,
                    order = orders.recv() => match order {
                        Some(Order::Decide(accept)) => break Some(accept),
                        Some(Order::Changed(keys)) => pending.extend(keys),
                        Some(Order::Presence(focused)) => wire.send(&Message::Presence { focused }).await?,
                        Some(Order::TakeMain) => peer.take_main_ordered.store(true, Ordering::SeqCst),
                        Some(Order::SendPeers) => {
                            let peers = with_peer_vault(app, peer, |vault| vault.sync_peers_to_send())?;
                            wire.send(&Message::Peers { peers }).await?;
                        }
                        Some(Order::Close) | None => return Err(Stop::Quiet),
                        Some(_) => {}
                    },
                    message = wire.next() => match message? {
                        Some(Message::Changed { records }) => {
                            check_stamps(app, peer, watch, &records);
                            held.extend(records);
                        }
                        Some(Message::Peers { peers }) => {
                            if take_peers(app, peer, &peers)? {
                                // Forgotten while the question was open: the
                                // question closes, nothing is applied.
                                update_link(app, &peer.session_id, |link| link.view.plan = None);
                                return Err(failure(PeerSyncFailure::Revoked));
                            }
                        }
                        Some(Message::TakeMain) => peer.take_main_heard.store(true, Ordering::SeqCst),
                        Some(Message::Time { now_ms: remote }) => {
                            time_report(app, peer, watch, remote, now_ms());
                        }
                        Some(Message::Presence { focused }) => presence(app, peer, focused),
                        Some(Message::Bye { reason }) => return Err(heard_bye(app, peer, &reason)),
                        None => return Err(Stop::Quiet),
                        Some(_) => {}
                    },
                }
            };
            update_link(app, &peer.session_id, |link| link.view.plan = None);
            if decision != Some(true) {
                let declined = decision == Some(false);
                if declined {
                    // "Decline" pauses exchange with this device until the
                    // user resumes it; a timeout only closes this session.
                    let _ = with_peer_vault(app, peer, |vault| {
                        vault.sync_journal_record(
                            &peer.name,
                            &summary,
                            SyncJournalOutcome::Declined,
                        )?;
                        vault.sync_pause_peer(&peer.device_id)
                    });
                }
                let _ = wire
                    .send(&Message::Bye {
                        reason: if declined { "declined" } else { "timeout" }.into(),
                    })
                    .await;
                // The user's own decision or silence: neither a failure to
                // show nor one to count against the address.
                return Err(Stop::Quiet);
            }
        }
    }

    // One backup per run, counted once it succeeded.
    let backup = app.hub().run.as_ref().is_some_and(|run| !run.backup_done);
    let peer_ref = PeerRef {
        device_id: peer.device_id.clone(),
        name: peer.name.clone(),
    };
    let mut dropped = false;
    let mut backed_up = false;
    let applied = app.with_vault(|vault| {
        // Checked in the same step as the write: a vault switched under the
        // session gets nothing.
        if vault.doc.id != peer.vault_id {
            return Ok(None);
        }
        // The question can be open for minutes: a device forgotten meanwhile
        // (here, or through another link) gets nothing.
        if vault.sync_state().trusted(&peer.remote_key).is_none() {
            let summary = vault
                .sync_preview_from(&incoming, &peer.device_id)
                .unwrap_or_default();
            vault.sync_journal_record(&peer.name, &summary, SyncJournalOutcome::Declined)?;
            dropped = true;
            return Ok(Some(None));
        }
        if backup {
            vault.sync_backup()?;
            backed_up = true;
        }
        let Some(batch) = vault.sync_apply(&incoming, &peer_ref)? else {
            return Ok(Some(None));
        };
        let keys = vault.sync_batch_keys(&batch.id);
        let notices = vault.sync_batch_notices(&batch.id);
        Ok(Some(Some((batch, keys, notices))))
    });
    if backed_up && let Some(run) = app.hub().run.as_mut() {
        run.backup_done = true;
    }
    let applied = match applied {
        Ok(applied) => applied,
        Err(err) => {
            // The journal tells that a batch came and failed; nothing of its content.
            let summary = app
                .with_vault(|vault| vault.sync_preview_from(&incoming, &peer.device_id))
                .unwrap_or_default();
            let _ = app.with_vault(|vault| {
                vault.sync_journal_record(&peer.name, &summary, SyncJournalOutcome::Failed)
            });
            return Err(err.into());
        }
    };
    if dropped {
        update_link(app, &peer.session_id, |link| link.view.plan = None);
        return Err(failure(PeerSyncFailure::Revoked));
    }
    let Some(applied) = applied else {
        return Err(Stop::Quiet);
    };
    update_link(app, &peer.session_id, |link| link.applied_once = true);
    if let Some((batch, keys, notices)) = applied {
        app.hub().last_activity = Some(std::time::Instant::now());
        app.emit(
            SYNCED_EVENT,
            VaultSyncedEvent {
                session_id: peer.session_id.clone(),
                batch,
            },
        );
        let fresh_after = i64::try_from(now_ms()).unwrap_or(i64::MAX) - FRESH_NOTICE_MS;
        let fresh: Vec<_> = notices
            .into_iter()
            .filter(|notice| {
                trade_core::model::vault::iso_to_millis(&notice.delivered_at) >= fresh_after
            })
            .collect();
        app.deliver(fresh);
        super::broadcast(app, &keys, Some(&peer.session_id));
    }
    Ok(())
}

// -- live -----------------------------------------------------------------------

async fn live<H: Host>(
    app: &H,
    wire: &mut Wire,
    orders: &mut mpsc::UnboundedReceiver<Order>,
    peer: &Peer,
    watch: &mut ClockWatch,
    mut pending: BTreeSet<String>,
) -> Flow<()> {
    app.hub().last_error = None;
    set_phase(app, &peer.session_id, SyncPhase::Live);
    if let Some(run) = app.hub().run.as_mut() {
        run.retries = 0;
    }
    // Handed over while this session was still pairing or exchanging.
    if peer.take_main_heard.swap(false, Ordering::SeqCst) {
        app.become_main();
    }
    let mut handing_over = hand_over_if_preferred(app, peer);
    if peer.take_main_ordered.swap(false, Ordering::SeqCst) {
        wire.send(&Message::TakeMain).await?;
        handing_over = true;
    }
    super::send_presence(&mut app.hub());
    // SendPeers orders are dropped while exchanging: say where we stand now.
    // The client sends this right after taking the main role, so it is also
    // the (wire-compatible) confirmation the main device waits for.
    let peers = with_peer_vault(app, peer, |vault| vault.sync_peers_to_send())?;
    wire.send(&Message::Peers { peers }).await?;

    let mut flush_at = (!pending.is_empty()).then(|| Instant::now() + DEBOUNCE);
    let mut ticker = tokio::time::interval(TIME_EVERY);
    wire.heard = Instant::now();
    loop {
        let flush = async {
            match flush_at {
                Some(at) => tokio::time::sleep_until(at).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            () = flush => {
                flush_at = None;
                let keys: Vec<String> = std::mem::take(&mut pending).into_iter().collect();
                for chunk in keys.chunks(RECORDS_PER_MESSAGE) {
                    let records = with_peer_vault(app, peer, |vault| vault.sync_records(chunk))?;
                    wire.send(&Message::Changed { records: records.into_iter().collect() }).await?;
                }
            }
            _ = ticker.tick() => {
                if wire.heard.elapsed() > SILENCE_LIMIT {
                    return Err(Stop::Quiet);
                }
                wire.send(&Message::Time { now_ms: now_ms() }).await?;
            }
            order = orders.recv() => match order {
                Some(Order::Changed(keys)) => {
                    pending.extend(keys);
                    flush_at.get_or_insert_with(|| Instant::now() + DEBOUNCE);
                }
                Some(Order::Presence(focused)) => wire.send(&Message::Presence { focused }).await?,
                Some(Order::TakeMain) => wire.send(&Message::TakeMain).await?,
                Some(Order::SendPeers) => {
                    let peers = with_peer_vault(app, peer, |vault| vault.sync_peers_to_send())?;
                    wire.send(&Message::Peers { peers }).await?;
                }
                Some(Order::Close) | None => {
                    // Edits still waiting for the debounce go out first. The
                    // vault may already be closed: then there is nothing to send.
                    let keys: Vec<String> = std::mem::take(&mut pending).into_iter().collect();
                    for chunk in keys.chunks(RECORDS_PER_MESSAGE) {
                        let Ok(records) = with_peer_vault(app, peer, |vault| vault.sync_records(chunk)) else {
                            break;
                        };
                        if wire.send(&Message::Changed { records: records.into_iter().collect() }).await.is_err() {
                            break;
                        }
                    }
                    let _ = wire.send(&Message::Bye { reason: "closed".into() }).await;
                    return Err(Stop::Quiet);
                }
                Some(_) => {}
            },
            message = wire.next() => {
                match message? {
                    Some(Message::Changed { records }) => {
                        check_stamps(app, peer, watch, &records);
                        let incoming: BTreeMap<String, RecordState> = records.into_iter().collect();
                        apply(app, wire, orders, peer, watch, incoming, None, &mut pending).await?;
                        // The question can take minutes: that is not silence.
                        wire.heard = Instant::now();
                        set_phase(app, &peer.session_id, SyncPhase::Live);
                        if !pending.is_empty() {
                            flush_at.get_or_insert_with(|| Instant::now() + DEBOUNCE);
                        }
                    }
                    Some(Message::Time { now_ms: remote }) => {
                        time_report(app, peer, watch, remote, now_ms());
                    }
                    Some(Message::Presence { focused }) => presence(app, peer, focused),
                    Some(Message::Peers { peers }) => {
                        if take_peers(app, peer, &peers)? {
                            return Err(failure(PeerSyncFailure::Revoked));
                        }
                        // The client that was handed the main role has taken
                        // it (it lists its devices from `live` onwards): now
                        // this device steps down.
                        if std::mem::take(&mut handing_over) {
                            app.demote();
                        }
                    }
                    Some(Message::TakeMain) => {
                        app.become_main();
                        // The answer the main device waits for before it steps down.
                        let peers = with_peer_vault(app, peer, |vault| vault.sync_peers_to_send())?;
                        wire.send(&Message::Peers { peers }).await?;
                    }
                    Some(Message::Bye { reason }) => return Err(heard_bye(app, peer, &reason)),
                    None => return Err(Stop::Quiet),
                    Some(_) => {}
                }
            }
        }
    }
}

/// On the main device: a client that prefers the main role gets it, once
/// its first exchange is through (never in the middle of a transfer). True
/// when the order went out: stepping down waits for the client's answer (its
/// first list of devices from `live`), because a client still in a question
/// only queues the hand-over; closing its link now would end the question.
fn hand_over_if_preferred<H: Host>(app: &H, peer: &Peer) -> bool {
    let prefer_here = app.settings().sync.prefer_main;
    let hand_over = {
        let hub = app.hub();
        let main = hub
            .run
            .as_ref()
            .is_some_and(|run| run.role == SyncRole::Main);
        main && !prefer_here
            && hub
                .links
                .get(&peer.session_id)
                .is_some_and(|link| link.prefer_main && !link.to_main)
    };
    if hand_over {
        super::send(app, &peer.session_id, Order::TakeMain);
    }
    hand_over
}
