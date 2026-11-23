//! The vault side of device-to-device sync: stamping local changes on every
//! save, describing the records to another device, applying and undoing what
//! comes back (docs/sync.md). The pure parts live in [`crate::sync`].

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::ipc_contract::SyncCutReason;
use crate::ipc_contract::security::SecurityLevel;
use crate::model::vault::{VaultDoc, now_iso};
use crate::service::peer_sync::TapSink;
use crate::service::{AppliedBatch, ExternalMerge, PeerRef, SyncTap, UndoReport, UnlockedVault};
use crate::storage::VaultFile;
use crate::sync::history::applied_of;
use crate::sync::journal::SyncJournalOutcome;
use crate::sync::manifest::Manifest;
use crate::sync::registry::Rule;
use crate::sync::repair::bad_record::unparsable_keys;
use crate::sync::repair::keep::{generated_key, mark_kept};
use crate::sync::state::{
    BATCH_DAYS, CutBy, MAX_BATCHES, REVOKED_GATE_DAYS, TOMBSTONE_DAYS, acked_through,
};
use crate::sync::track::Tracker;
use crate::sync::view::{self, split_key};
use crate::sync::{BatchEntry, RecordState, Stamp, SyncBatch, SyncSummary, TrustedPeer};

/// Device id on batches that came from the external copy.
const EXTERNAL_COPY_DEVICE: &str = "external-copy";

/// Rotated backups kept before the first batch of each session.
pub const PRESYNC_BACKUPS: usize = 3;

fn ser_err(err: serde_json::Error) -> Error {
    Error::Serialize(err.to_string())
}

fn day_ms(days: u64) -> u64 {
    days.saturating_mul(24 * 3600 * 1000)
}

/// An automatic reminder is one per source (kind + entity id), however its id
/// or its key was made. Reminders of the old build (random id, schedule-only
/// key) group with the new ones by their source.
fn reminder_identity(reminder: &crate::model::planning::Reminder) -> Option<String> {
    match &reminder.source {
        Some(source) => Some(crate::planning::auto_reminders::source_identity(source)),
        None => (!reminder.source_key.is_empty()).then(|| reminder.source_key.clone()),
    }
}

/// A stamp that sorts after every entry held for this device (and key): the
/// clock lives here, so the trust scenarios ask the vault for it.
pub(crate) fn stamp_past_known_peer(
    vault: &mut UnlockedVault,
    device_id: &str,
    public_key: Option<&str>,
) -> Stamp {
    for known in crate::sync::trust::known_stamps(&vault.doc.sync, device_id, public_key) {
        vault.clock.observe(&known);
    }
    vault.clock.tick()
}

/// Whether notices made inside the core are in Russian: the repair scenario
/// asks the vault, the field is private to `service`.
pub(crate) fn notice_in_russian(vault: &UnlockedVault) -> bool {
    vault.notice_ru
}

/// The document without the sync bookkeeping, serialized.
pub(crate) fn doc_value(vault: &mut UnlockedVault) -> Result<Value> {
    let sync = std::mem::take(&mut vault.doc.sync);
    let value = serde_json::to_value(&vault.doc);
    vault.doc.sync = sync;
    value.map_err(ser_err)
}

/// Start stamping changes from the document as it is now.
pub(crate) fn start_tracking(vault: &mut UnlockedVault) -> Result<()> {
    if vault.sandbox {
        return Ok(());
    }
    let value = doc_value(vault)?;
    vault.tracker = Some(Tracker::capture(&value));
    // A poisoned stamp (a record from a wrong clock) stays stored, but the
    // clock starts after the latest one that is plausible.
    let limit = crate::sync::stamp::now_ms().saturating_add(crate::sync::stamp::MAX_FUTURE_MS);
    if let Some(latest) = vault.doc.sync.latest_stamp_within(limit).cloned() {
        vault.clock.observe(&latest);
    }
    Ok(())
}

/// Our own peer entries (a revocation or trust made while the clock was
/// poisoned) stamped beyond the limit would be skipped by every peer.
/// Give them a fresh bounded stamp. True when something changed.
pub(crate) fn restamp_poisoned_peers(vault: &mut UnlockedVault) -> bool {
    let now = crate::sync::stamp::now_ms();
    let limit = now.saturating_add(crate::sync::stamp::MAX_FUTURE_MS);
    let mine = vault.clock.device().to_string();
    let mut changed = false;
    for index in 0..vault.doc.sync.peers.len() {
        let entry = &vault.doc.sync.peers[index];
        if entry.stamp.wall <= limit || entry.stamp.device != mine {
            continue;
        }
        let stamp = vault.clock.tick_at(now);
        let entry = &mut vault.doc.sync.peers[index];
        if entry.revoked.is_some() {
            entry.revoked = Some(stamp.clone());
        }
        entry.stamp = stamp;
        changed = true;
    }
    changed
}

/// The trusted devices as they go on the wire. Re-stamps our own entries
/// that a poisoned clock pushed beyond the limit, here (the single exit
/// toward peers) and not only at open, because the device id is set after
/// open and an entry can also arrive in a copy.
pub fn sync_peers_to_send(vault: &mut UnlockedVault) -> Result<Vec<TrustedPeer>> {
    if restamp_poisoned_peers(vault) {
        vault.persist()?;
    }
    Ok(vault.doc.sync.peers.clone())
}

/// Stamp what changed since the last save; the changed keys.
pub(crate) fn track_changes(vault: &mut UnlockedVault) -> Result<Vec<String>> {
    if vault.sandbox {
        return Ok(Vec::new());
    }
    let value = doc_value(vault)?;
    let UnlockedVault {
        tracker,
        doc,
        clock,
        announce,
        ..
    } = vault;
    match tracker {
        Some(tracker) => {
            let changed = tracker.stamp(&value, &mut doc.sync, clock);
            announce.extend(changed.iter().cloned());
            Ok(changed)
        }
        None => {
            *tracker = Some(Tracker::capture(&value));
            Ok(Vec::new())
        }
    }
}

/// A deletion in a capped log (the change tracker reads a missing
/// entry there as a trim): drops its stamps, leaves a tombstone, and
/// offers the key to a live session.
pub(crate) fn sync_forget_log_record(vault: &mut UnlockedVault, key: &str) -> Result<()> {
    if vault.sandbox {
        return Ok(());
    }
    track_changes(vault)?;
    let deleted = vault.clock.tick();
    vault.doc.sync.stamps.remove(key);
    vault.doc.sync.tombstones.insert(key.to_string(), deleted);
    vault.announce.insert(key.to_string());
    Ok(())
}

/// This installation's id, for the stamps of its changes.
pub fn set_device(vault: &mut UnlockedVault, device_id: &str) {
    vault.clock.set_device(device_id);
}

/// Report every later save's changed keys to `tap` (`None` stops).
pub fn set_sync_tap(vault: &mut UnlockedVault, tap: Option<Arc<dyn SyncTap>>) {
    if !vault.sandbox {
        vault.sync_tap = tap.clone();
        vault
            .session
            .set_change_sink(tap.map(|tap| Arc::new(TapSink(tap))));
    }
}

/// The vault's sync bookkeeping (peers, batches).
pub fn sync_state(vault: &UnlockedVault) -> &crate::sync::SyncState {
    &vault.doc.sync
}

/// [`sync_manifest`] and the newest stamp of this device's clock at
/// that moment: what the peer will have seen once this exchange is applied
/// (see [`sync_mark_synced`]).
pub fn sync_manifest_marked(
    vault: &mut UnlockedVault,
    union_since_ms: Option<u64>,
) -> Result<(Manifest, Stamp)> {
    let manifest = crate::usecases::sync::read::sync_manifest(vault, union_since_ms)?;
    Ok((manifest, vault.clock.last()))
}

/// [`sync_manifest_marked`] for a peer protected at `peer` (see
/// [`sync_manifest_for`]); the third item counts the records left out.
pub fn sync_manifest_marked_for(
    vault: &mut UnlockedVault,
    union_since_ms: Option<u64>,
    peer: SecurityLevel,
) -> Result<(Manifest, Stamp, usize)> {
    let (manifest, withheld) =
        crate::usecases::sync::read::sync_manifest_for(vault, union_since_ms, peer)?;
    Ok((manifest, vault.clock.last(), withheld))
}

/// Merge records from another device and save. `None` when nothing
/// changed here; otherwise the batch, kept for undo.
pub fn sync_apply(
    vault: &mut UnlockedVault,
    incoming: &BTreeMap<String, RecordState>,
    peer: &PeerRef,
) -> Result<Option<AppliedBatch>> {
    apply_batch(vault, incoming, peer, true)
}

/// [`sync_apply`]; `carry` off is the earlier dedupe (the copy
/// made later goes with its edits), which the tests keep as the other
/// side of a pair of different versions.
pub(crate) fn apply_batch(
    vault: &mut UnlockedVault,
    incoming: &BTreeMap<String, RecordState>,
    peer: &PeerRef,
    carry: bool,
) -> Result<Option<AppliedBatch>> {
    apply_batch_from(vault, incoming, peer, carry, false)
}

/// [`apply_batch`]; `external` is the records of an external copy
/// (`merge_from`): fields without a stamp on either side are reported.
/// Which value is picked does not depend on it.
pub(crate) fn apply_batch_from(
    vault: &mut UnlockedVault,
    incoming: &BTreeMap<String, RecordState>,
    peer: &PeerRef,
    carry: bool,
    external: bool,
) -> Result<Option<AppliedBatch>> {
    crate::sync::manifest_view::check_incoming(incoming)?;
    vault.sync_check_level(incoming)?;
    track_changes(vault)?;
    for state in incoming.values() {
        if let Some(latest) = state.latest() {
            vault.clock.observe(latest);
        }
    }
    let mut value = doc_value(vault)?;
    let records = view::records(&value);
    // A deletion old enough to be compacted, for a key this device knows
    // nothing about, is not news: the senders had it acked everywhere
    // else. Storing it would only restart its age.
    let incoming = &after_wipe(vault, without_compacted(vault, incoming, &records));
    let seen = vault.doc.sync.seen_by.get(&peer.device_id).cloned();
    let plan_of = |batch: &BTreeMap<String, RecordState>| {
        if external {
            crate::sync::plan::plan_external(batch, |key| {
                crate::sync::manifest_view::record_state(&vault.doc, &records, key)
            })
        } else {
            crate::sync::plan::plan_seen(
                batch,
                |key| crate::sync::manifest_view::record_state(&vault.doc, &records, key),
                seen.as_ref(),
            )
        }
    };
    let mut plan = plan_of(incoming);
    if plan.changes.is_empty() {
        return Ok(None);
    }
    let mut applied_value = value.clone();
    for (key, change) in &plan.changes {
        view::write(&mut applied_value, key, change.after.value.as_ref());
    }
    view::trim(&mut applied_value);
    // A record the cap cuts at once is not news: plan again without it,
    // so it is neither counted nor listed.
    let kept = view::records(&applied_value);
    let cut: Vec<&String> = plan
        .changes
        .iter()
        .filter(|(key, change)| change.after.value.is_some() && !kept.contains_key(*key))
        .map(|(key, _)| key)
        .collect();
    if !cut.is_empty() {
        let rest: BTreeMap<String, RecordState> = incoming
            .iter()
            .filter(|(key, _)| !cut.contains(key))
            .map(|(key, state)| (key.clone(), state.clone()))
            .collect();
        plan = plan_of(&rest);
        if plan.changes.is_empty() {
            return Ok(None);
        }
        applied_value = value.clone();
        for (key, change) in &plan.changes {
            view::write(&mut applied_value, key, change.after.value.as_ref());
        }
        view::trim(&mut applied_value);
    }
    let base = std::mem::replace(&mut value, applied_value);
    // Rebuilding the typed document is the schema check: a record that
    // does not fit is left out on its own, the rest still applies.
    let mut doc: VaultDoc = match serde_json::from_value(value.clone()) {
        Ok(doc) => doc,
        Err(err) => {
            let bad = unparsable_keys(&base, &plan);
            if bad.is_empty() {
                return Err(Error::InvalidInput(format!("sync: {err}")));
            }
            let (left_out, rest): (BTreeMap<_, _>, BTreeMap<_, _>) = incoming
                .iter()
                .map(|(key, state)| (key.clone(), state.clone()))
                .partition(|(key, _)| bad.contains(key));
            let summary = plan_of(&left_out).summary;
            // The skip is told only if the rest applies: on failure the
            // journal goes back with the rolled-back document.
            let journal = vault.doc.sync.journal.clone();
            let journaled =
                crate::usecases::sync::repair::journal_skipped(vault, &peer.name, &summary);
            let mut outcome = match apply_batch_from(vault, &rest, peer, carry, external) {
                Ok(outcome) => outcome,
                Err(err) => {
                    vault.doc.sync.journal = journal;
                    return Err(err);
                }
            };
            match outcome.as_mut() {
                Some(applied) => applied.skipped = bad,
                None if journaled => vault.persist()?,
                None => {}
            }
            return Ok(outcome);
        }
    };
    // Cloned, not taken: the old document must stay whole for a rollback.
    doc.sync = vault.doc.sync.clone();
    crate::sync::apply::stamp_changes(&mut doc, &plan.changes);
    // Log entries the cap cut off keep no stamps.
    let kept = view::records(&value);
    doc.sync.stamps.retain(|key, _| kept.contains_key(key));
    // The merged document is complete: swap it in. Every step after this
    // can fail; on failure the old document goes back (atomic batch).
    let tracker = vault.tracker.clone();
    let announce = vault.announce.clone();
    let before = swap_doc(vault, doc);
    match finish_batch(vault, plan, peer, carry) {
        Ok(applied) => Ok(Some(applied)),
        Err(err) => {
            // Tracker and queued announcements first, then the document;
            // the swap cannot fail, so the batch's own error is the one
            // the caller sees. Keys the batch queued would otherwise go
            // to a live session for changes that never happened.
            vault.tracker = tracker;
            vault.announce = announce;
            swap_doc(vault, before);
            Err(err)
        }
    }
}

/// Put `doc` in place of the open document; the old one comes back. It
/// cannot fail, so a rollback never hides the error that caused it.
pub(crate) fn swap_doc(vault: &mut UnlockedVault, doc: VaultDoc) -> VaultDoc {
    std::mem::replace(&mut vault.doc, doc)
}

/// What follows the swap of a merged document: settle it, journal the
/// batch, repair, save once.
fn finish_batch(
    vault: &mut UnlockedVault,
    plan: crate::sync::SyncPlan,
    peer: &PeerRef,
    carry: bool,
) -> Result<AppliedBatch> {
    // A union of two devices' caches can overshoot the caps: thin it the
    // way a refresh does, before the baseline is taken, so the dropped
    // points are neither stamped nor sent as deletions.
    if plan
        .changes
        .keys()
        .any(|key| key.starts_with("price_cache:") || key.starts_with("fx_cache:"))
    {
        crate::usecases::money::rates::thin_rate_caches(vault);
    }
    // The typed document may serialize a little differently from the
    // merged JSON (defaults, number forms): the tracker starts from what
    // a save will actually see, or the next save would restamp the batch.
    // Envelopes that arrived whole (made by a device with no journal) get
    // their opening entry; a merged journal's total is re-derived. Both
    // are the same on every device, so they are not stamped as edits.
    crate::usecases::household::envelopes::settle_envelopes(vault);
    let settled = doc_value(vault)?;
    if let Some(tracker) = vault.tracker.as_mut() {
        tracker.rebase(&settled);
    }

    let batch = SyncBatch {
        id: uuid::Uuid::new_v4().to_string(),
        peer_device: peer.device_id.clone(),
        peer_name: peer.name.clone(),
        applied_at: now_iso(),
        summary: plan.summary,
        entries: plan
            .changes
            .into_iter()
            .map(|(key, change)| BatchEntry {
                key,
                before: change.before,
                after: change.after.marks(),
            })
            .collect(),
        undone_at: None,
    };
    let applied = applied_of(&batch);
    crate::usecases::sync::sync_journal::journal_push(
        vault,
        &applied.peer_name,
        &applied.summary,
        SyncJournalOutcome::Applied,
        Some(applied.id.clone()),
    );
    vault.doc.sync.batches.push(batch);
    prune_batches(vault);
    compact_tombstones(vault);
    let queued = vault.announce.clone();
    // Both devices may have run the same automation or import before
    // they met; the copy made later goes (as an ordinary deletion).
    dedupe_generated(vault, &applied.id, carry)?;
    // The rules a merge cannot keep (account tree, envelopes of deleted
    // goals, dangling settings): `repair.rs`.
    crate::usecases::sync::repair::repair_invariants(vault, Some(&applied.id))?;
    crate::usecases::money::portfolio::recompute_holdings(vault);
    persist_then_announce(vault, &queued)?;
    Ok(applied)
}

/// Save, announcing only what the batch itself queued after the write
/// went through: a save announces the queue before it writes, and a failed
/// batch is rolled back, so its keys would reach a live session for
/// changes that never happened. `queued` is what was waiting before.
///
/// Written at once (no save hook), the batch keys are announced right
/// after `persist` succeeds, to the sync tap if one is set. Batched
/// (dirty, a hook waits), they join the queue and go with the next save.
fn persist_then_announce(
    vault: &mut UnlockedVault,
    queued: &std::collections::BTreeSet<String>,
) -> Result<()> {
    let batch_keys: Vec<String> = vault.announce.difference(queued).cloned().collect();
    vault.announce.clone_from(queued);
    vault.persist()?;
    if vault.sandbox || vault.session.is_dirty() {
        vault.announce.extend(batch_keys);
    } else {
        vault.announce_keys_to_tap(batch_keys);
    }
    Ok(())
}

/// `incoming` without the deletions that were already compacted: tombstone
/// only, older than `TOMBSTONE_DAYS`, for a key with no value and no
/// tombstone here.
pub(crate) fn without_compacted(
    vault: &UnlockedVault,
    incoming: &BTreeMap<String, RecordState>,
    records: &BTreeMap<String, Value>,
) -> BTreeMap<String, RecordState> {
    let cutoff = crate::sync::stamp::now_ms().saturating_sub(day_ms(TOMBSTONE_DAYS));
    incoming
        .iter()
        .filter(|(key, state)| {
            let old_deletion = state.value.is_none()
                && state.stamps.is_none()
                && state
                    .tombstone
                    .as_ref()
                    .is_some_and(|stamp| stamp.wall < cutoff);
            let unknown =
                !records.contains_key(*key) && !vault.doc.sync.tombstones.contains_key(*key);
            !(old_deletion && unknown)
        })
        .map(|(key, state)| (key.clone(), state.clone()))
        .collect()
}

/// `incoming` without the crypto-module records a wipe here has already
/// outlived (spec E12).
///
/// A device that was offline while the module was wiped still holds every
/// record of it; the ordinary merge would take them back, because this
/// vault no longer has anything to compare them against — a record it
/// never saw leaves no tombstone behind, and quotes union silently. The
/// mark the wipe left says when it happened, so those records are dropped
/// here and whatever that device made *after* the wipe still arrives.
/// Silently: a peer handing over its old records is the normal course of
/// a wipe, not a fault to report.
pub(crate) fn after_wipe(
    vault: &UnlockedVault,
    incoming: BTreeMap<String, RecordState>,
) -> BTreeMap<String, RecordState> {
    let wipes = vault
        .doc
        .crypto_wiped_at
        .iter()
        .map(|wipe| (wipe.id.as_str(), wipe.at.as_str()));
    match crate::sync::crypto_gate::newest_wipe(wipes, &vault.doc.sync.stamps) {
        Some(mark) => crate::sync::crypto_gate::without_wiped(incoming, &mark).0,
        None => incoming,
    }
}

/// Whether an external copy fully opens under this open vault's key (no
/// KDF runs): a truncated, foreign or cheaper copy is refused. Nothing is
/// written.
pub fn probe_external(
    vault: &UnlockedVault,
    remote: &[u8],
) -> Result<crate::storage::SnapshotFormat> {
    let key = vault
        .key
        .as_deref()
        .ok_or_else(|| Error::Crypto("the vault has no open key".into()))?;
    crate::vault_io::import::probe_open(remote, key)
}

/// Read another copy of this vault (its sealed bytes) without locking,
/// writing or syncing anything. Only for reading records out of it.
pub(crate) fn open_copy(
    vault: &UnlockedVault,
    remote: &[u8],
    password: &str,
) -> Result<UnlockedVault> {
    let (plaintext, fmt, _key) = crate::crypto::open_bytes_keyed(remote, password.as_bytes())?;
    let decoded = crate::storage::decode_file(remote, &plaintext, fmt)?;
    Ok(UnlockedVault {
        path: vault.path.clone(),
        fmt,
        doc: decoded.doc,
        base_currency: vault.base_currency.clone(),
        notice_ru: vault.notice_ru,
        password: Zeroizing::new(String::new()),
        _lock: crate::storage::VaultLock::detached(),
        // Nothing in the copy is ever written back.
        sandbox: true,
        tracker: None,
        clock: vault.clock.clone(),
        sync_tap: None,
        announce: std::collections::BTreeSet::new(),
        key: None,
        session: crate::kernel::session::Write::sandboxed(),
        device_keys: Default::default(),
    })
}

/// Bring the records of `other` (the same vault, another copy) into this
/// one the way a peer would: what only `other` has or changed later
/// arrives, what only this vault has stays. Only records travel: `other`'s
/// batches, journal, peers and pauses are never taken.
pub fn merge_from(
    vault: &mut UnlockedVault,
    other: &mut UnlockedVault,
    name: &str,
) -> Result<Option<AppliedBatch>> {
    if vault.doc.id != other.doc.id {
        return Err(Error::InvalidInput(
            "merge: the copy is another vault".into(),
        ));
    }
    let mine = crate::usecases::sync::read::sync_manifest(vault, None)?;
    let theirs = crate::usecases::sync::read::sync_manifest(other, None)?;
    let (wanted, _) = crate::usecases::sync::read::sync_diff(vault, &mine, &theirs);
    let incoming = crate::usecases::sync::read::sync_records(other, &wanted)?;
    let peer = PeerRef {
        device_id: EXTERNAL_COPY_DEVICE.into(),
        name: name.into(),
    };
    apply_batch_from(vault, &incoming, &peer, true, true)
}

/// Merge an external copy (sealed bytes, same password) in; the result is
/// written. `Unmergeable` when it does not open or is another vault.
pub fn merge_external_copy(
    vault: &mut UnlockedVault,
    remote: &[u8],
    password: &str,
    name: &str,
) -> Result<ExternalMerge> {
    let Ok(mut copy) = open_copy(vault, remote, password) else {
        return Ok(ExternalMerge::Unmergeable);
    };
    if copy.doc.id != vault.doc.id {
        return Ok(ExternalMerge::Unmergeable);
    }
    let applied = merge_from(vault, &mut copy, name)?;
    vault.persist_now()?;
    Ok(ExternalMerge::Done(applied))
}

/// The external copy is the newer one and this vault has no changes of its
/// own (the file equals the last sync): take its data. The sync
/// bookkeeping stays this device's own: the copy may come from a device
/// with other batches, a journal and pauses.
pub fn take_external_copy(
    vault: &mut UnlockedVault,
    remote: &[u8],
    password: &str,
) -> Result<ExternalMerge> {
    let Ok(mut copy) = open_copy(vault, remote, password) else {
        return Ok(ExternalMerge::Unmergeable);
    };
    if copy.doc.id != vault.doc.id {
        return Ok(ExternalMerge::Unmergeable);
    }
    // Stamps and tombstones describe the records, so they come with the
    // data; the rest is this device's own bookkeeping. Trusted peers
    // travel between devices, so they are merged by their own rules.
    let mut sync = std::mem::take(&mut vault.doc.sync);
    let remote_sync = std::mem::take(&mut copy.doc.sync);
    sync.take_records_from(remote_sync, crate::sync::stamp::now_ms());
    vault.doc = std::mem::take(&mut copy.doc);
    vault.doc.sync = sync;
    start_tracking(vault)?;
    crate::usecases::money::stores::ensure_cash_wallet(vault, "Cash")?;
    crate::usecases::planning::passive::ensure_default_categories(vault);
    crate::usecases::workflow::records::disable_incomplete_workflows(
        vault,
        crate::calendar::now_secs(),
    );
    crate::usecases::household::attachments_cleanup::prune_attachments(vault);
    crate::usecases::sync::sync_journal::scrub_sync_secrets(vault);
    crate::usecases::money::portfolio::recompute_holdings(vault);
    vault.persist_now()?;
    Ok(ExternalMerge::Done(None))
}

/// The vault was just handed to a new device: trust it for sync, keep it
/// in the history and leave a notice every linked device shows (so a
/// transfer the owner did not make does not go unseen). Returns the notice.
pub fn sync_record_transfer(
    vault: &mut UnlockedVault,
    device_id: &str,
    device_name: &str,
    public_key: &str,
    ru: bool,
) -> Result<crate::model::notify::NoticeRecord> {
    let at = now_iso();
    let id = uuid::Uuid::new_v4().to_string();
    vault
        .doc
        .sync
        .transfers
        .push(crate::sync::state::TransferRecord {
            id: id.clone(),
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            at: at.clone(),
        });
    crate::usecases::sync::sync_journal::journal_push(
        vault,
        device_name,
        &SyncSummary::default(),
        SyncJournalOutcome::Sent,
        None,
    );
    let extra = vault
        .doc
        .sync
        .transfers
        .len()
        .saturating_sub(crate::sync::state::MAX_TRANSFERS);
    vault.doc.sync.transfers.drain(..extra);
    let (title, body) = if ru {
        (
            "Хранилище передано на другое устройство".to_string(),
            format!(
                "Копия хранилища ушла на «{device_name}». Если это были не вы, смените пароль."
            ),
        )
    } else {
        (
            "The vault was handed to another device".to_string(),
            format!(
                "A copy of the vault went to «{device_name}». If it was not you, change the password."
            ),
        )
    };
    let notice = crate::model::notify::notice(
        crate::model::notify::NoticeCategory::Security,
        format!("transfer:{id}"),
        &title,
        body,
        Some("/?settings=sync".into()),
        at,
    );
    crate::usecases::planning::notifications::log_notices(vault, vec![notice.clone()])?;
    crate::usecases::sync::trust::sync_trust_peer(vault, device_id, device_name, public_key)?;
    Ok(notice)
}

/// Put back what a batch changed, for every record nobody changed since.
/// The undo is a new local change: it reaches the other devices too.
pub fn sync_undo(vault: &mut UnlockedVault, batch_id: &str) -> Result<UndoReport> {
    track_changes(vault)?;
    let Some(index) = vault
        .doc
        .sync
        .batches
        .iter()
        .position(|batch| batch.id == batch_id)
    else {
        return Err(Error::NotFound(format!("sync batch {batch_id}")));
    };
    if vault.doc.sync.batches[index].undone_at.is_some() {
        return Err(Error::InvalidInput("this sync is already undone".into()));
    }
    let entries = vault.doc.sync.batches[index].entries.clone();
    let mut value = doc_value(vault)?;
    let records = view::records(&value);
    let mut report = UndoReport::default();
    // Log records the batch added: removed with a tombstone, or the peer
    // would send them back.
    let mut forgotten = Vec::new();
    // Records the dedupe of this batch had removed and the undo brings
    // back: they get the «keep» mark.
    let mut brought_back = Vec::new();
    for entry in &entries {
        let current = crate::sync::manifest_view::record_state(&vault.doc, &records, &entry.key);
        let union =
            split_key(&entry.key).is_some_and(|(coll, _)| matches!(coll.rule, Rule::Union { .. }));
        if !union && current.same_marks(&entry.after) {
            view::write(&mut value, &entry.key, entry.before.value.as_ref());
            let added_log = entry.before.value.is_none()
                && split_key(&entry.key)
                    .is_some_and(|(coll, _)| matches!(coll.rule, Rule::Log { .. }));
            if added_log {
                forgotten.push(entry.key.clone());
            }
            if entry.before.value.is_some() && entry.after.tombstone.is_some() {
                brought_back.push(entry.key.clone());
            }
            report.restored += 1;
        } else if let Some((coll, inner)) = split_key(&entry.key) {
            let shown = current
                .value
                .as_ref()
                .map(|value| view::label(coll, value))
                .unwrap_or_else(|| inner.to_string());
            report.skipped.push(shown);
        }
    }
    // Nothing was restored: the batch stays undoable, the reply says why.
    if report.restored == 0 {
        return Ok(report);
    }
    let mut doc: VaultDoc = serde_json::from_value(value)
        .map_err(|err| Error::InvalidInput(format!("sync undo: {err}")))?;
    doc.sync = std::mem::take(&mut vault.doc.sync);
    doc.sync.batches[index].undone_at = Some(now_iso());
    vault.doc = doc;
    mark_kept(&mut vault.doc, &brought_back, reminder_identity);
    for key in &forgotten {
        sync_forget_log_record(vault, key)?;
    }
    // An undone mark must not make a booked moment fire again.
    crate::usecases::workflow::runs::keep_schedule_marks(vault);
    crate::usecases::money::portfolio::recompute_holdings(vault);
    vault.persist()?;
    Ok(report)
}

pub(crate) fn prune_batches(vault: &mut UnlockedVault) {
    let cutoff = crate::sync::stamp::now_ms().saturating_sub(day_ms(BATCH_DAYS as u64));
    vault.doc.sync.batches.retain(|batch| {
        u64::try_from(crate::model::vault::iso_to_millis(&batch.applied_at)).unwrap_or(0) >= cutoff
    });
    let extra = vault.doc.sync.batches.len().saturating_sub(MAX_BATCHES);
    vault.doc.sync.batches.drain(..extra);
}

/// Forget deletions old enough that every device that counts has them.
/// A deletion counts from the later of its own stamp and the time it
/// reached this device: one that arrives late is not compacted at once.
/// Counting devices are the trusted ones and those forgotten less than
/// `REVOKED_GATE_DAYS` ago; each must have acked the deletion (its manifest
/// held it, see `sync_ack_tombstones`). No ack yet means keep.
pub(crate) fn compact_tombstones(vault: &mut UnlockedVault) {
    let now = crate::sync::stamp::now_ms();
    let cutoff = now.saturating_sub(day_ms(TOMBSTONE_DAYS));
    let revoked_cutoff = now.saturating_sub(day_ms(REVOKED_GATE_DAYS));
    let own = vault.clock.device();
    let sync = &mut vault.doc.sync;
    let acks: Vec<u64> = sync
        .peers
        .iter()
        // This device never acks to itself (old vaults hold such an entry).
        .filter(|peer| !crate::sync::state::is_own_device(own, &peer.device_id))
        .filter(|peer| {
            peer.revoked
                .as_ref()
                .is_none_or(|revoked| revoked.wall >= revoked_cutoff)
        })
        .map(|peer| {
            sync.tombstone_acks
                .get(&peer.device_id)
                .copied()
                .unwrap_or(0)
        })
        .collect();
    let seen = &sync.tombstones_seen;
    sync.tombstones.retain(|key, stamp| {
        let counted = stamp.wall.max(seen.get(key).copied().unwrap_or(0));
        counted >= cutoff || acks.iter().any(|acked| *acked < counted)
    });
    let tombstones = &sync.tombstones;
    sync.tombstones_seen
        .retain(|key, _| tombstones.contains_key(key));
}

pub(crate) fn created_of(vault: &UnlockedVault, key: &str) -> Stamp {
    vault
        .doc
        .sync
        .stamps
        .get(key)
        .map(|stamps| stamps.created.clone())
        .unwrap_or_default()
}

/// Keep one of each operation an automation or an import made twice
/// (once per device): the one created first, then the smaller id. What
/// the other copy was edited to, later than the kept one, moves into the
/// kept one (per field, with the stamps it had), so no edit is lost.
///
/// A workflow run may book several operations for one planned moment, so
/// there the earliest operation picks the winning run, and everything of
/// that run stays, and so does everything made on the same device (a
/// separate run of its own). Only another run on another device goes. An
/// operation without a stamp is never deleted.
///
/// Removals and carried fields are saved in the batch `batch_id`, so its
/// undo puts them back. `carry` off is the earlier rule (tests only).
pub(crate) fn dedupe_generated(
    vault: &mut UnlockedVault,
    batch_id: &str,
    carry: bool,
) -> Result<()> {
    struct Winner {
        rank: (Stamp, String),
        run_id: Option<String>,
    }

    // (key of the record that goes, key of the record it passes edits to)
    let mut removals: Vec<(String, Option<String>)> = Vec::new();

    // Kept by hand (the user undid a merge): never removed, never a
    // reason to remove its twin.
    let kept_tx: HashSet<&str> = vault
        .doc
        .transactions
        .iter()
        .filter(|tx| tx.meta().keep)
        .map(|tx| tx.meta().id.as_str())
        .collect();
    let mut best: HashMap<String, Winner> = HashMap::new();
    for tx in &vault.doc.transactions {
        let meta = tx.meta();
        let Some((natural, run)) = generated_key(&meta.origin) else {
            continue;
        };
        let rank = (
            created_of(vault, &format!("transactions:{}", meta.id)),
            meta.id.clone(),
        );
        let winner = Winner {
            rank: rank.clone(),
            run_id: run.map(str::to_string),
        };
        match best.get_mut(&natural) {
            Some(kept) if rank < kept.rank => *kept = winner,
            Some(_) => {}
            None => {
                best.insert(natural, winner);
            }
        }
    }
    for tx in &vault.doc.transactions {
        let meta = tx.meta();
        let Some((natural, run)) = generated_key(&meta.origin) else {
            continue;
        };
        let Some(winner) = best.get(&natural) else {
            continue;
        };
        if winner.rank.1 == meta.id
            || kept_tx.contains(meta.id.as_str())
            || kept_tx.contains(winner.rank.1.as_str())
        {
            continue;
        }
        let stamp = created_of(vault, &format!("transactions:{}", meta.id));
        // No stamp: nobody can tell which copy is later, so none goes.
        let keep = stamp.device.is_empty()
            // Only workflow operations travel in runs.
            || (run.is_some()
                && (run == winner.run_id.as_deref() || stamp.device == winner.rank.0.device));
        if !keep {
            // Operations of different runs are different bookings: only
            // a plain copy of one operation takes the other's edits.
            let into = run
                .is_none()
                .then(|| format!("transactions:{}", winner.rank.1));
            removals.push((format!("transactions:{}", meta.id), into));
        }
    }

    // An automatic reminder is one per source: `reminder_identity`.
    let mut first: HashMap<String, (Stamp, String)> = HashMap::new();
    for reminder in &vault.doc.reminders {
        let Some(group) = reminder_identity(reminder) else {
            continue;
        };
        let rank = (
            created_of(vault, &format!("reminders:{}", reminder.id)),
            reminder.id.clone(),
        );
        first
            .entry(group)
            .and_modify(|kept| {
                if rank < *kept {
                    *kept = rank.clone();
                }
            })
            .or_insert(rank);
    }
    // The loser's later-edited fields (done, muted, snoozed, edited by
    // hand, the time) move into the kept copy, each by its own stamp.
    let kept_reminders: HashSet<&str> = vault
        .doc
        .reminders
        .iter()
        .filter(|reminder| reminder.keep)
        .map(|reminder| reminder.id.as_str())
        .collect();
    for reminder in &vault.doc.reminders {
        if let Some(group) = reminder_identity(reminder)
            && let Some((_, kept)) = first.get(&group)
            && *kept != reminder.id
            && !kept_reminders.contains(reminder.id.as_str())
            && !kept_reminders.contains(kept.as_str())
        {
            removals.push((
                format!("reminders:{}", reminder.id),
                Some(format!("reminders:{kept}")),
            ));
        }
    }
    remove_duplicates(vault, removals, batch_id, carry)
}

/// Delete the records of `removals` as ordinary deletions (a tombstone
/// each), after moving their later-edited fields into the record named
/// beside them. Everything is recorded in the batch for undo and offered
/// to a live session; the change tracker is rebased so none of it is
/// stamped a second time.
pub(crate) fn remove_duplicates(
    vault: &mut UnlockedVault,
    removals: Vec<(String, Option<String>)>,
    batch_id: &str,
    carry: bool,
) -> Result<()> {
    if removals.is_empty() {
        return Ok(());
    }
    let mut value = doc_value(vault)?;
    let records = view::records(&value);
    let mut sync = vault.doc.sync.clone();
    let mut touched: BTreeMap<String, RecordState> = BTreeMap::new();
    let mut after: BTreeMap<String, RecordState> = BTreeMap::new();
    // (operation that goes, operation that stays) for `repoint_tx`.
    let mut repoints: Vec<(String, String)> = Vec::new();
    for (loser, into) in removals {
        if let Some(winner) = &into
            && let (Some(old), Some(new)) = (
                loser.strip_prefix("transactions:"),
                winner.strip_prefix("transactions:"),
            )
        {
            repoints.push((old.to_string(), new.to_string()));
        }
        touched.entry(loser.clone()).or_insert_with(|| {
            crate::sync::manifest_view::record_state(&vault.doc, &records, &loser)
        });
        if carry
            && let Some(winner) = &into
            && let Some((coll, _)) = split_key(winner)
            && let (Some(from), Some(to)) = (view::read(&value, &loser), view::read(&value, winner))
        {
            touched.entry(winner.clone()).or_insert_with(|| {
                crate::sync::manifest_view::record_state(&vault.doc, &records, winner)
            });
            let loser_fields = view::split(coll, &from);
            let mut kept_fields = view::split(coll, &to);
            if loser_fields.tag == kept_fields.tag {
                let loser_stamps = sync.stamps.get(&loser).cloned().unwrap_or_default();
                let mut kept_stamps = sync.stamps.get(winner).cloned().unwrap_or_default();
                let mut moved = false;
                for (name, stamp) in &loser_stamps.fields {
                    let Some(edited) = loser_fields.fields.get(name) else {
                        continue;
                    };
                    // The key never moves, and an equal value has
                    // nothing to carry.
                    if coll.key.iter().any(|path| path.ends_with(name.as_str()))
                        || kept_fields.fields.get(name) == Some(edited)
                        || stamp <= kept_stamps.field(name)
                    {
                        continue;
                    }
                    kept_fields.fields.insert(name.clone(), edited.clone());
                    kept_stamps.fields.insert(name.clone(), stamp.clone());
                    moved = true;
                }
                if moved {
                    view::write(&mut value, winner, Some(&view::join(coll, &kept_fields)));
                    sync.stamps.insert(winner.clone(), kept_stamps.clone());
                    after.insert(
                        winner.clone(),
                        RecordState {
                            value: None,
                            stamps: Some(kept_stamps),
                            tombstone: None,
                        },
                    );
                    vault.announce.insert(winner.clone());
                }
            }
        }
        view::write(&mut value, &loser, None);
        sync.stamps.remove(&loser);
        let deleted = vault.clock.tick();
        sync.tombstones.insert(loser.clone(), deleted.clone());
        after.insert(
            loser.clone(),
            RecordState {
                value: None,
                stamps: None,
                tombstone: Some(deleted),
            },
        );
        vault.announce.insert(loser);
    }
    let mut doc: VaultDoc = serde_json::from_value(value)
        .map_err(|err| Error::InvalidInput(format!("sync dedupe: {err}")))?;
    if let Some(batch) = sync.batches.iter_mut().find(|batch| batch.id == batch_id) {
        for (key, marks) in after {
            match batch.entries.iter_mut().find(|entry| entry.key == key) {
                Some(entry) => entry.after = marks,
                None => batch.entries.push(BatchEntry {
                    before: touched.remove(&key).unwrap_or_default(),
                    key,
                    after: marks,
                }),
            }
        }
    }
    doc.sync = sync;
    vault.doc = doc;
    let settled = doc_value(vault)?;
    if let Some(tracker) = vault.tracker.as_mut() {
        tracker.rebase(&settled);
    }
    // After the rebase: the moved references are an edit of this device,
    // stamped and sent on, not part of the merge.
    crate::usecases::sync::repair::repoint_removed_copies(vault, &repoints, batch_id)
}

// -- trusted devices ------------------------------------------------------

/// A vault file that arrived by transfer carries the sender's sync
/// bookkeeping verbatim. Keep the record metadata and the trusted devices,
/// drop everything device-local (undo batches, journal, transfers,
/// pauses, cut-offs, deletion arrival times) and the peers' last-sync times.
pub fn adopt_received_copy(vault: &mut UnlockedVault) -> Result<()> {
    let received = std::mem::take(&mut vault.doc.sync);
    let mut fresh = crate::sync::SyncState::default();
    fresh.take_records_from(received, crate::sync::stamp::now_ms());
    for peer in &mut fresh.peers {
        peer.last_synced_at = None;
    }
    vault.doc.sync = fresh;
    vault.persist()
}

/// Take in another device's list of trusted devices.
pub fn sync_merge_peers(vault: &mut UnlockedVault, remote: &[TrustedPeer]) -> Result<bool> {
    let now = crate::sync::stamp::now_ms();
    for peer in remote {
        vault.clock.observe_at(&peer.stamp, now);
    }
    let changed = vault.doc.sync.merge_peers(remote, now);
    if changed {
        vault.persist()?;
    }
    Ok(changed)
}

/// Pause exchange with a device (the user declined its list of changes).
/// Kept in this vault only; it is never sent to other devices.
pub fn sync_pause_peer(vault: &mut UnlockedVault, device_id: &str) -> Result<()> {
    if vault.doc.sync.paused.insert(device_id.to_string()) {
        vault.persist()?;
    }
    Ok(())
}

/// "Resume": exchange with the device may run again.
pub fn sync_resume_peer(vault: &mut UnlockedVault, device_id: &str) -> Result<()> {
    if vault.doc.sync.paused.remove(device_id) {
        vault.persist()?;
    }
    Ok(())
}

pub fn sync_is_paused(vault: &UnlockedVault, device_id: &str) -> bool {
    vault.doc.sync.paused.contains(device_id)
}

/// The device told us it cut this one off (`reason`). Returns whether this
/// is news (a new device or another reason); only news is saved.
pub fn sync_cut_by(
    vault: &mut UnlockedVault,
    device_id: &str,
    reason: SyncCutReason,
) -> Result<bool> {
    if vault
        .doc
        .sync
        .cut_by
        .get(device_id)
        .is_some_and(|known| known.reason == reason)
    {
        return Ok(false);
    }
    vault.doc.sync.cut_by.insert(
        device_id.to_string(),
        CutBy {
            reason,
            at: now_iso(),
        },
    );
    vault.persist()?;
    Ok(true)
}

/// The way back was taken (an exchange, a pairing): the device may be
/// dialed again. Returns whether there was anything to clear.
pub fn sync_clear_cut(vault: &mut UnlockedVault, device_id: &str) -> Result<bool> {
    if vault.doc.sync.cut_by.remove(device_id).is_none() {
        return Ok(false);
    }
    vault.persist()?;
    Ok(true)
}

pub fn sync_cut_reason(vault: &UnlockedVault, device_id: &str) -> Option<SyncCutReason> {
    vault.doc.sync.cut_by.get(device_id).map(|cut| cut.reason)
}

/// Record what the manifest `theirs` of `device_id` vouches for: how far
/// our deletions are all in it. Call it only for an exchange that was
/// applied: a declined or failed one proves nothing. Never goes down.
pub fn sync_ack_tombstones(
    vault: &mut UnlockedVault,
    device_id: &str,
    theirs: &Manifest,
) -> Result<()> {
    let ours = crate::usecases::sync::read::sync_manifest(vault, None)?;
    let sync = &vault.doc.sync;
    let deletions: Vec<(u64, &str, u64)> = sync
        .tombstones
        .iter()
        .filter_map(|(key, stamp)| {
            let counted = stamp
                .wall
                .max(sync.tombstones_seen.get(key).copied().unwrap_or(0));
            Some((counted, key.as_str(), *ours.get(key)?))
        })
        .collect();
    let previous = sync.tombstone_acks.get(device_id).copied().unwrap_or(0);
    let acked = acked_through(deletions, theirs, previous);
    if acked != previous {
        vault
            .doc
            .sync
            .tombstone_acks
            .insert(device_id.to_string(), acked);
        vault.persist()?;
    }
    Ok(())
}

/// Note a finished exchange with a device (not synced as a change). `mark`
/// is the clock stamp taken with the manifest of that exchange
/// ([`sync_manifest_marked`]): the device has seen our edits up to it.
/// Only an applied exchange may pass it. Never goes down.
pub fn sync_mark_synced(
    vault: &mut UnlockedVault,
    device_id: &str,
    mark: Option<&Stamp>,
) -> Result<()> {
    if let Some(mark) = mark {
        let seen = vault
            .doc
            .sync
            .seen_by
            .entry(device_id.to_string())
            .or_default();
        if mark > seen {
            *seen = mark.clone();
        }
    }
    if let Some(peer) = vault
        .doc
        .sync
        .peers
        .iter_mut()
        .find(|peer| peer.device_id == device_id)
    {
        peer.last_synced_at = Some(now_iso());
    }
    vault.persist()
}

// -- backups --------------------------------------------------------------

pub(crate) fn presync_path(vault: &UnlockedVault, index: usize) -> PathBuf {
    let mut name = vault.path.file_name().unwrap_or_default().to_os_string();
    if index == 0 {
        name.push(".presync.bak");
    } else {
        name.push(format!(".presync.{index}.bak"));
    }
    vault.path.with_file_name(name)
}

/// Copy the vault file aside before the first batch of a session,
/// keeping the last [`PRESYNC_BACKUPS`].
pub fn sync_backup(vault: &mut UnlockedVault) -> Result<PathBuf> {
    if vault.sandbox {
        return Ok(presync_path(vault, 0));
    }
    for index in (1..PRESYNC_BACKUPS).rev() {
        let from = presync_path(vault, index - 1);
        if from.exists() {
            std::fs::rename(&from, presync_path(vault, index))
                .map_err(|err| Error::Io(err.to_string()))?;
        }
    }
    let target = presync_path(vault, 0);
    // The copy is of the file: batched changes go to it first.
    vault.flush_pending()?;
    std::fs::copy(&vault.path, &target).map_err(|err| Error::Io(err.to_string()))?;
    Ok(target)
}

/// Delete every pre-sync backup (a missing file is fine).
pub(crate) fn remove_sync_backups(vault: &UnlockedVault) {
    for index in 0..PRESYNC_BACKUPS {
        let _ = std::fs::remove_file(presync_path(vault, index));
    }
}

/// Whether a pre-sync backup exists that can be restored: a copy sealed
/// under a password cannot be opened by a vault that has none now.
pub fn has_sync_backup(vault: &UnlockedVault) -> bool {
    let path = presync_path(vault, 0);
    path.exists() && (vault.is_encrypted() || !crate::crypto::file_is_encrypted(&path))
}

/// The emergency exit: the whole vault as it was before the last
/// session's first batch. Saved as new local changes, so it reaches the
/// other devices like any edit.
pub fn sync_restore_backup(vault: &mut UnlockedVault) -> Result<()> {
    let path = presync_path(vault, 0);
    if !has_sync_backup(vault) {
        return Err(Error::NotFound("no backup from before a sync".into()));
    }
    let (doc, _) = VaultFile::open(&path, &vault.password)?;
    track_changes(vault)?;
    vault.replace_doc(doc)
}
