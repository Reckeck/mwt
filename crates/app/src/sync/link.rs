//! One connection to another device, from the handshake to the end of the
//! session: hello and checks, pairing, the first exchange, then live changes.
//!
//! The vault's mutex is taken only for short, synchronous steps and never
//! held across an `.await` (rule 3).

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio::time::Instant;
use trade_core::error::{Error, PeerSyncFailure, Result};
use trade_core::ipc_contract::{
    SyncClockEvent, SyncConfirmReason, SyncOfferEvent, SyncPairingEvent, SyncPendingPlan,
    SyncPhase, SyncPlanEvent, SyncProgress, SyncRole, SyncSessionView, VaultSyncedEvent,
};
use trade_core::service::PeerRef;
use trade_core::sync::RecordState;
use trade_core::sync::manifest::{Manifest, diff};
use trade_core::sync::protocol::{
    Hello, MANIFEST_PER_MESSAGE, Message, PROTOCOL_VERSION, RECORDS_PER_MESSAGE, decode, encode,
};
use trade_core::sync::registry::UNION_DAYS;
use trade_core::sync::session::{Channel, ClockWatch, Handshake, Skew, skew_of};
use trade_core::sync::stamp::now_ms;

use super::host::Host;
use super::{
    CLOCK_EVENT, Link, OFFER_EVENT, Order, PAIRING_EVENT, PLAN_EVENT, SYNCED_EVENT, set_phase,
    set_progress, update_link,
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const PAIRING_TIMEOUT: Duration = Duration::from_secs(300);
const DECISION_TIMEOUT: Duration = Duration::from_secs(600);
const DEBOUNCE: Duration = Duration::from_secs(1);
const TIME_EVERY: Duration = Duration::from_secs(30);
const SILENCE_LIMIT: Duration = Duration::from_secs(95);
/// Notices older than this that arrive by sync are logged, not shown.
const FRESH_NOTICE_MS: i64 = 3600 * 1000;

/// How a session ends: quietly (closed, locked, the other side left) or with
/// an error the user is told about.
enum Stop {
    Quiet,
    Failed(Error),
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
            if let Some(bytes) = self.channel.open(&frame)? {
                return Ok(Some(decode(&bytes)?));
            }
        }
    }
}

// -- entry ----------------------------------------------------------------------

/// `dialer`: this device opened the connection. Which side is the main
/// device comes from the hellos: a client dials the main device, or the main
/// device calls back a client that cannot reach it.
pub(crate) async fn run<H: Host>(
    app: H,
    stream: TcpStream,
    addr: SocketAddr,
    generation: u64,
    dialer: bool,
) {
    let session_id = uuid::Uuid::new_v4().to_string();
    let mut to_main = false;
    let result = session(
        &app,
        stream,
        addr,
        generation,
        dialer,
        &session_id,
        &mut to_main,
    )
    .await;
    let peer = app
        .hub()
        .links
        .remove(&session_id)
        .map(|link| link.view.name);
    if let Err(Stop::Failed(err)) = &result {
        if matches!(
            err,
            Error::PeerSync(
                PeerSyncFailure::OtherVault | PeerSyncFailure::Rejected | PeerSyncFailure::Channel
            )
        ) {
            super::note_failure(&app, addr.ip());
        }
        super::emit_error(&app, err, peer.as_deref());
    }
    if to_main {
        app.main_lost(generation);
    }
    super::emit_status(&app);
}

struct Peer {
    session_id: String,
    device_id: String,
    name: String,
    remote_key: String,
}

async fn session<H: Host>(
    app: &H,
    stream: TcpStream,
    addr: SocketAddr,
    generation: u64,
    dialer: bool,
    session_id: &str,
    to_main_out: &mut bool,
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
    };
    let remote_key = wire.channel.remote_hex();

    // -- hello --
    let (vault_id, trusted, revoked) = app.with_vault(|vault| {
        Ok((
            vault.doc.id.clone(),
            vault.sync_state().trusted(&remote_key).is_some(),
            vault.sync_state().is_revoked(&remote_key),
        ))
    })?;
    let settings = app.settings();
    let (main_here, rank, instance, unreached) = {
        let mut hub = app.hub();
        match hub.run.as_mut().filter(|run| run.generation == generation) {
            Some(run) if run.role == SyncRole::Main => {
                let rank = run.next_rank;
                run.next_rank += 1;
                (true, rank, run.instance.clone(), false)
            }
            Some(run) => (false, 0, run.instance.clone(), run.unreached),
            None => return Err(Stop::Quiet),
        }
    };
    wire.send(&Message::Hello(Hello {
        protocol: PROTOCOL_VERSION,
        format: trade_core::VAULT_FORMAT_VERSION,
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
    if theirs.protocol != PROTOCOL_VERSION || theirs.format != trade_core::VAULT_FORMAT_VERSION {
        return Err(failure(PeerSyncFailure::VersionMismatch));
    }
    let declined = app
        .hub()
        .run
        .as_ref()
        .is_some_and(|run| run.declined.contains(&theirs.device_id));
    if theirs.device_id == settings.device_id || declined {
        return Err(Stop::Quiet);
    }
    // Exactly one side of a link is the main device; two main devices sort
    // it out through discovery.
    if main_here == theirs.main {
        return Err(Stop::Quiet);
    }
    let to_main = theirs.main;
    *to_main_out = to_main;
    let callback = dialer == main_here;
    if revoked {
        let _ = wire
            .send(&Message::Bye {
                reason: "revoked".into(),
            })
            .await;
        return Err(failure(PeerSyncFailure::Revoked));
    }
    let mut watch = ClockWatch::new(theirs.now_ms, now_ms());
    match skew_of(watch.offset()) {
        Skew::Refuse(_) => return Err(failure(PeerSyncFailure::ClockSkew)),
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
    {
        let mut guard = app.hub();
        let hub = &mut *guard;
        let Some(run) = hub.run.as_mut().filter(|run| run.generation == generation) else {
            return Err(Stop::Quiet);
        };
        if to_main {
            // A client called back while it was still looking.
            if run.role == SyncRole::Main {
                return Err(Stop::Quiet);
            }
            run.role = SyncRole::Client;
            run.rank = theirs.rank;
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
                    plan: None,
                },
                orders: orders_in,
                to_main,
                callback,
                instance: theirs.instance.clone(),
                prefer_main: theirs.prefer_main,
                applied_once: false,
            },
        );
    }
    super::emit_status(app);
    let peer = Peer {
        session_id: session_id.to_string(),
        device_id: theirs.device_id.clone(),
        name: theirs.device_name.clone(),
        remote_key,
    };

    let mut reason = None;
    if !trusted || theirs.needs_pairing {
        pair(app, &mut wire, &mut orders, &peer).await?;
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
    let show_code = |app: &H| {
        update_link(app, &peer.session_id, |link| {
            link.view.phase = SyncPhase::Pairing;
            link.view.pairing_code = Some(code.clone());
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
                        return Err(failure(PeerSyncFailure::Rejected));
                    }
                    mine = true;
                }
                Some(Order::Close) | None => return Err(Stop::Quiet),
                Some(_) => {}
            },
            message = wire.next() => match message? {
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
                Some(Message::Bye { .. }) | None => return Err(Stop::Quiet),
                Some(_) => {}
            },
        }
        if mine && theirs {
            update_link(app, &peer.session_id, |link| link.view.pairing_code = None);
            app.with_vault(|vault| {
                vault.sync_trust_peer(&peer.device_id, &peer.name, &peer.remote_key)
            })?;
            return Ok(());
        }
    }
}

/// Do not offer this device again until sync restarts.
fn decline<H: Host>(app: &H, peer: &Peer) {
    if let Some(run) = app.hub().run.as_mut() {
        run.declined.insert(peer.device_id.clone());
    }
}

// -- first exchange -------------------------------------------------------------

fn check_stamps<H: Host>(
    app: &H,
    peer: &Peer,
    watch: &ClockWatch,
    records: &[(String, RecordState)],
) {
    let now = now_ms();
    if records
        .iter()
        .filter_map(|(_, state)| state.latest())
        .any(|stamp| watch.check_stamp(stamp.wall, now).is_some())
    {
        mark_unreliable(app, peer, watch);
    }
}

fn mark_unreliable<H: Host>(app: &H, peer: &Peer, watch: &ClockWatch) {
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

async fn send_records<H: Host>(app: &H, wire: &mut Wire, keys: &[String]) -> Flow<()> {
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
        let records = app.with_vault(|vault| vault.sync_records(chunk))?;
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
    let peers = app.with_vault(|vault| Ok(vault.sync_state().peers.clone()))?;
    wire.send(&Message::Peers { peers }).await?;
    let since = now_ms().saturating_sub(UNION_DAYS * 24 * 3600 * 1000);
    let mine = app.with_vault(|vault| vault.sync_manifest(Some(since)))?;
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
    while !(asked && answered && received) {
        tokio::select! {
            order = orders.recv() => match order {
                Some(Order::Changed(keys)) => pending.extend(keys),
                Some(Order::Presence(focused)) => wire.send(&Message::Presence { focused }).await?,
                Some(Order::Close) | None => return Err(Stop::Quiet),
                Some(_) => {}
            },
            message = wire.next() => match message? {
                Some(Message::Peers { peers }) => {
                    let revoked = app.with_vault(|vault| {
                        vault.sync_merge_peers(&peers)?;
                        Ok(vault.sync_state().is_revoked(&peer.remote_key))
                    })?;
                    if revoked {
                        return Err(failure(PeerSyncFailure::Revoked));
                    }
                }
                Some(Message::Manifest { entries, last }) => {
                    theirs.extend(entries);
                    if last {
                        let (want, _) = diff(&mine, &theirs);
                        total = u32::try_from(want.len()).unwrap_or(u32::MAX);
                        if total > 0 {
                            set_progress(app, &peer.session_id, Some(SyncProgress { received: 0, total }));
                        }
                        wire.send(&Message::Request { keys: want }).await?;
                        asked = true;
                    }
                }
                Some(Message::Request { keys }) => {
                    send_records(app, wire, &keys).await?;
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
                    if watch.observe(remote, now_ms()).is_some() {
                        mark_unreliable(app, peer, watch);
                    }
                }
                Some(Message::Presence { focused }) => presence(app, peer, focused),
                Some(Message::Bye { .. }) | None => return Err(Stop::Quiet),
                Some(_) => {}
            },
        }
    }
    set_progress(app, &peer.session_id, None);
    apply(app, wire, orders, peer, watch, incoming, reason, pending).await?;
    app.with_vault(|vault| vault.sync_mark_synced(&peer.device_id))?;
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
#[allow(clippy::too_many_arguments)]
async fn apply<H: Host>(
    app: &H,
    wire: &mut Wire,
    orders: &mut mpsc::UnboundedReceiver<Order>,
    peer: &Peer,
    watch: &mut ClockWatch,
    mut incoming: BTreeMap<String, RecordState>,
    reason: Option<SyncConfirmReason>,
    pending: &mut BTreeSet<String>,
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
        let summary = app.with_vault(|vault| vault.sync_preview(&incoming))?;
        if !summary.is_empty() {
            update_link(app, &peer.session_id, |link| {
                link.view.phase = SyncPhase::AwaitingConfirm;
                link.view.plan = Some(SyncPendingPlan {
                    reason,
                    summary: summary.clone(),
                });
            });
            app.emit(
                PLAN_EVENT,
                SyncPlanEvent {
                    session_id: peer.session_id.clone(),
                    peer_name: peer.name.clone(),
                    reason,
                    summary,
                },
            );
            let deadline = tokio::time::sleep(DECISION_TIMEOUT);
            tokio::pin!(deadline);
            let accept = loop {
                tokio::select! {
                    () = &mut deadline => break false,
                    order = orders.recv() => match order {
                        Some(Order::Decide(accept)) => break accept,
                        Some(Order::Changed(keys)) => pending.extend(keys),
                        Some(Order::Presence(focused)) => wire.send(&Message::Presence { focused }).await?,
                        Some(Order::Close) | None => return Err(Stop::Quiet),
                        Some(_) => {}
                    },
                    message = wire.next() => match message? {
                        Some(Message::Changed { records }) => {
                            check_stamps(app, peer, watch, &records);
                            incoming.extend(records);
                        }
                        Some(Message::Time { now_ms: remote }) => {
                            if watch.observe(remote, now_ms()).is_some() {
                                mark_unreliable(app, peer, watch);
                            }
                        }
                        Some(Message::Presence { focused }) => presence(app, peer, focused),
                        Some(Message::Bye { .. }) | None => return Err(Stop::Quiet),
                        Some(_) => {}
                    },
                }
            };
            update_link(app, &peer.session_id, |link| link.view.plan = None);
            if !accept {
                let _ = wire
                    .send(&Message::Bye {
                        reason: "declined".into(),
                    })
                    .await;
                return Err(failure(PeerSyncFailure::Rejected));
            }
        }
    }

    let backup = {
        let mut hub = app.hub();
        hub.run
            .as_mut()
            .is_some_and(|run| !std::mem::replace(&mut run.backup_done, true))
    };
    let peer_ref = PeerRef {
        device_id: peer.device_id.clone(),
        name: peer.name.clone(),
    };
    let applied = app.with_vault(|vault| {
        if backup {
            vault.sync_backup()?;
        }
        let Some(batch) = vault.sync_apply(&incoming, &peer_ref)? else {
            return Ok(None);
        };
        let keys = vault.sync_batch_keys(&batch.id);
        let notices = vault.sync_batch_notices(&batch.id);
        Ok(Some((batch, keys, notices)))
    })?;
    app.hub().last_activity = Some(std::time::Instant::now());
    update_link(app, &peer.session_id, |link| link.applied_once = true);
    if let Some((batch, keys, notices)) = applied {
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
    hand_over_if_preferred(app, peer);
    super::send_presence(&mut app.hub());

    let mut flush_at = (!pending.is_empty()).then(|| Instant::now() + DEBOUNCE);
    let mut ticker = tokio::time::interval(TIME_EVERY);
    let mut last_heard = Instant::now();
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
                    let records = app.with_vault(|vault| vault.sync_records(chunk))?;
                    wire.send(&Message::Changed { records: records.into_iter().collect() }).await?;
                }
            }
            _ = ticker.tick() => {
                if last_heard.elapsed() > SILENCE_LIMIT {
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
                    let peers = app.with_vault(|vault| Ok(vault.sync_state().peers.clone()))?;
                    wire.send(&Message::Peers { peers }).await?;
                }
                Some(Order::Close) | None => {
                    let _ = wire.send(&Message::Bye { reason: "closed".into() }).await;
                    return Err(Stop::Quiet);
                }
                Some(_) => {}
            },
            message = wire.next() => {
                last_heard = Instant::now();
                match message? {
                    Some(Message::Changed { records }) => {
                        check_stamps(app, peer, watch, &records);
                        let incoming: BTreeMap<String, RecordState> = records.into_iter().collect();
                        apply(app, wire, orders, peer, watch, incoming, None, &mut pending).await?;
                        if !pending.is_empty() {
                            flush_at.get_or_insert_with(|| Instant::now() + DEBOUNCE);
                        }
                    }
                    Some(Message::Time { now_ms: remote }) => {
                        if watch.observe(remote, now_ms()).is_some() {
                            mark_unreliable(app, peer, watch);
                        }
                    }
                    Some(Message::Presence { focused }) => presence(app, peer, focused),
                    Some(Message::Peers { peers }) => {
                        let revoked = app.with_vault(|vault| {
                            vault.sync_merge_peers(&peers)?;
                            Ok(vault.sync_state().is_revoked(&peer.remote_key))
                        })?;
                        if revoked {
                            return Err(failure(PeerSyncFailure::Revoked));
                        }
                    }
                    Some(Message::TakeMain) => app.become_main(),
                    Some(Message::Bye { .. }) | None => return Err(Stop::Quiet),
                    Some(_) => {}
                }
            }
        }
    }
}

/// On the main device: a client that prefers the main role gets it, once
/// its first exchange is through (never in the middle of a transfer).
fn hand_over_if_preferred<H: Host>(app: &H, peer: &Peer) {
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
    if !hand_over {
        return;
    }
    super::send(app, &peer.session_id, Order::TakeMain);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        app.demote();
    });
}
