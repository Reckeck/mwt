//! The vault side of device-to-device sync: stamping local changes on every
//! save, describing the records to another device, applying and undoing what
//! comes back (docs/sync.md). The pure parts live in [`crate::sync`].

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zeroize::Zeroizing;

use super::UnlockedVault;
use super::auto_reminders::source_identity;
use crate::error::{Error, Result};
use crate::ipc_contract::{AfterApply, StoreBalanceChange, SyncCutReason};
use crate::model::planning::Reminder;
use crate::model::transaction::TxOrigin;
use crate::model::vault::{VaultDoc, now_iso};
use crate::storage::VaultFile;
use crate::sync::journal::SyncJournalOutcome;
use crate::sync::manifest::{Manifest, digest};
use crate::sync::registry::Rule;
use crate::sync::state::{
    BATCH_DAYS, CutBy, MAX_BATCHES, REVOKED_GATE_DAYS, TOMBSTONE_DAYS, acked_through,
};
use crate::sync::track::Tracker;
use crate::sync::view::{self, record_key, split_key};
use crate::sync::{BatchEntry, RecordState, Stamp, SyncBatch, SyncSummary, TrustedPeer};

/// Told the keys of every local save, so a live session can send them on.
pub trait SyncTap: Send + Sync {
    fn changed(&self, keys: &[String]);
}

/// The device a batch came from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeerRef {
    pub device_id: String,
    pub name: String,
}

/// A batch just applied, as the UI shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct AppliedBatch {
    pub id: String,
    pub peer_name: String,
    pub applied_at: String,
    pub summary: SyncSummary,
    #[serde(default)]
    pub undone_at: Option<String>,
    /// Not a batch: the vault was handed to this device from here.
    #[serde(default)]
    pub transferred_to: Option<String>,
}

/// What an undo did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct UndoReport {
    pub restored: u32,
    /// Labels of records changed again since the batch: left as they are.
    pub skipped: Vec<String>,
}

/// What came of reading an external copy into the open vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalMerge {
    /// The copy was taken in; the batch it produced, if it changed anything.
    Done(Option<AppliedBatch>),
    /// The copy does not open with this password or is another vault: the
    /// caller falls back to asking the user which side to keep.
    Unmergeable,
}

/// Device id on batches that came from the external copy.
const EXTERNAL_COPY_DEVICE: &str = "external-copy";

/// Rotated backups kept before the first batch of each session.
pub const PRESYNC_BACKUPS: usize = 3;

fn ser_err(err: serde_json::Error) -> Error {
    Error::Serialize(err.to_string())
}

impl UnlockedVault {
    /// The document without the sync bookkeeping, serialized.
    fn doc_value(&mut self) -> Result<Value> {
        let sync = std::mem::take(&mut self.doc.sync);
        let value = serde_json::to_value(&self.doc);
        self.doc.sync = sync;
        value.map_err(ser_err)
    }

    /// Start stamping changes from the document as it is now.
    pub(super) fn start_tracking(&mut self) -> Result<()> {
        if self.sandbox {
            return Ok(());
        }
        let value = self.doc_value()?;
        self.tracker = Some(Tracker::capture(&value));
        // A poisoned stamp (a record from a wrong clock) stays stored, but the
        // clock starts after the latest one that is plausible.
        let limit = crate::sync::stamp::now_ms().saturating_add(crate::sync::stamp::MAX_FUTURE_MS);
        if let Some(latest) = self.doc.sync.latest_stamp_within(limit).cloned() {
            self.clock.observe(&latest);
        }
        Ok(())
    }

    /// Our own peer entries (a revocation or trust made while the clock was
    /// poisoned) stamped beyond the limit would be skipped by every peer.
    /// Give them a fresh bounded stamp. True when something changed.
    fn restamp_poisoned_peers(&mut self) -> bool {
        let now = crate::sync::stamp::now_ms();
        let limit = now.saturating_add(crate::sync::stamp::MAX_FUTURE_MS);
        let mine = self.clock.device().to_string();
        let mut changed = false;
        for index in 0..self.doc.sync.peers.len() {
            let entry = &self.doc.sync.peers[index];
            if entry.stamp.wall <= limit || entry.stamp.device != mine {
                continue;
            }
            let stamp = self.clock.tick_at(now);
            let entry = &mut self.doc.sync.peers[index];
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
    pub fn sync_peers_to_send(&mut self) -> Result<Vec<TrustedPeer>> {
        if self.restamp_poisoned_peers() {
            self.persist()?;
        }
        Ok(self.doc.sync.peers.clone())
    }

    /// Stamp what changed since the last save; the changed keys.
    pub(super) fn track_changes(&mut self) -> Result<Vec<String>> {
        if self.sandbox {
            return Ok(Vec::new());
        }
        let value = self.doc_value()?;
        let Self {
            tracker,
            doc,
            clock,
            announce,
            ..
        } = self;
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
    pub(super) fn sync_forget_log_record(&mut self, key: &str) -> Result<()> {
        if self.sandbox {
            return Ok(());
        }
        self.track_changes()?;
        let deleted = self.clock.tick();
        self.doc.sync.stamps.remove(key);
        self.doc.sync.tombstones.insert(key.to_string(), deleted);
        self.announce.insert(key.to_string());
        Ok(())
    }

    /// This installation's id, for the stamps of its changes.
    pub fn set_device(&mut self, device_id: &str) {
        self.clock.set_device(device_id);
    }

    /// Report every later save's changed keys to `tap` (`None` stops).
    pub fn set_sync_tap(&mut self, tap: Option<Arc<dyn SyncTap>>) {
        if !self.sandbox {
            self.sync_tap = tap;
        }
    }

    /// The vault's sync bookkeeping (peers, batches).
    pub fn sync_state(&self) -> &crate::sync::SyncState {
        &self.doc.sync
    }

    fn state_of(&self, records: &BTreeMap<String, Value>, key: &str) -> RecordState {
        let union = split_key(key).is_some_and(|(coll, _)| matches!(coll.rule, Rule::Union { .. }));
        let value = records.get(key).cloned();
        if union {
            return RecordState {
                value,
                stamps: None,
                tombstone: None,
            };
        }
        match value {
            Some(value) => RecordState {
                value: Some(value),
                stamps: Some(self.doc.sync.stamps.get(key).cloned().unwrap_or_default()),
                tombstone: None,
            },
            None => RecordState {
                value: None,
                stamps: None,
                tombstone: self.doc.sync.tombstones.get(key).cloned(),
            },
        }
    }

    /// A record as the digest sees it: what the devices share, not what
    /// stays on one (a flow's cursor).
    fn shared_state(mut state: RecordState, key: &str) -> RecordState {
        if let (Some((coll, _)), Some(value)) = (split_key(key), state.value.as_mut()) {
            *value = view::shareable(coll, value);
        }
        state
    }

    /// One digest per record and deletion. `union_since_ms` leaves out cached
    /// prices and rates older than that (a first sync sends 90 days).
    pub fn sync_manifest(&mut self, union_since_ms: Option<u64>) -> Result<Manifest> {
        self.track_changes()?;
        let value = self.doc_value()?;
        let records = view::records(&value);
        let keeps_fx = self.fx_history_filter();
        let mut manifest = Manifest::new();
        for key in records.keys() {
            if !self.wants_cache_key(key, &keeps_fx) {
                continue;
            }
            if let (Some(since), Some((coll, _))) = (union_since_ms, split_key(key))
                && let Some(field) = coll.age
                && let Some(Value::String(at)) = records[key].get(field)
                && u64::try_from(crate::model::vault::iso_to_millis(at)).unwrap_or(0) < since
            {
                continue;
            }
            manifest.insert(
                key.clone(),
                digest(&Self::shared_state(self.state_of(&records, key), key)),
            );
        }
        for key in self.doc.sync.tombstones.keys() {
            if !records.contains_key(key) {
                manifest.insert(
                    key.clone(),
                    digest(&Self::shared_state(self.state_of(&records, key), key)),
                );
            }
        }
        Ok(manifest)
    }

    /// Whether a cache point is worth syncing here: rates of a currency that
    /// is not in use are thinned to one point right after a merge (the rule
    /// of `thin_rate_caches`), so asking for or offering them only repeats
    /// the same list in every session. Other keys always qualify.
    fn wants_cache_key(&self, key: &str, keeps_fx: &impl Fn(&str) -> bool) -> bool {
        match key.strip_prefix("fx_cache:") {
            Some(point) => keeps_fx(point.split_once('|').map_or(point, |(code, _)| code)),
            None => true,
        }
    }

    /// `(to_request, to_send)` for two manifests: [`diff`] without the cache
    /// points this device would thin away as soon as they arrive.
    ///
    /// [`diff`]: crate::sync::manifest::diff
    pub fn sync_diff(&self, mine: &Manifest, theirs: &Manifest) -> (Vec<String>, Vec<String>) {
        let keeps_fx = self.fx_history_filter();
        let (mut request, send) = crate::sync::manifest::diff(mine, theirs);
        request.retain(|key| self.wants_cache_key(key, &keeps_fx));
        (request, send)
    }

    /// The full state of each asked record (absent keys come back empty).
    pub fn sync_records(&mut self, keys: &[String]) -> Result<BTreeMap<String, RecordState>> {
        self.track_changes()?;
        let value = self.doc_value()?;
        let records = view::records(&value);
        Ok(keys
            .iter()
            .filter(|key| split_key(key).is_some())
            .map(|key| {
                let mut state = self.state_of(&records, key);
                if let (Some((coll, _)), Some(stamps)) = (split_key(key), state.stamps.as_mut()) {
                    crate::sync::merge::with_legacy_heads(coll, stamps);
                }
                (key.clone(), state)
            })
            .collect())
    }

    /// Refuse records filed under a key they do not carry, or under a
    /// collection that does not sync: a broken or hostile peer must not be
    /// able to overwrite one record with another.
    fn check_incoming(incoming: &BTreeMap<String, RecordState>) -> Result<()> {
        for (key, state) in incoming {
            let Some((coll, inner)) = split_key(key) else {
                return Err(Error::InvalidInput(format!("sync: unknown record {key}")));
            };
            if let Some(value) = &state.value
                && coll.rule != Rule::Single
                && record_key(coll, value).as_deref() != Some(inner)
            {
                return Err(Error::InvalidInput(format!(
                    "sync: record does not match {key}"
                )));
            }
        }
        Ok(())
    }

    /// What applying `incoming` would change, without changing anything.
    pub fn sync_preview(
        &mut self,
        incoming: &BTreeMap<String, RecordState>,
    ) -> Result<SyncSummary> {
        Self::check_incoming(incoming)?;
        self.track_changes()?;
        let value = self.doc_value()?;
        let records = view::records(&value);
        Ok(crate::sync::plan::plan(incoming, |key| self.state_of(&records, key)).summary)
    }

    /// Capital and the balances of the stores the batch moves, as they would
    /// be once applied: the batch goes into an in-memory copy of the vault
    /// (a sandbox writes nothing), this vault is not touched.
    pub fn sync_preview_after(
        &mut self,
        incoming: &BTreeMap<String, RecordState>,
    ) -> Result<Option<AfterApply>> {
        Self::check_incoming(incoming)?;
        self.track_changes()?;
        let mut copy = self.sandbox();
        let peer = PeerRef {
            device_id: String::new(),
            name: String::new(),
        };
        // A sandbox never persists, mirrors or announces: the copy takes the
        // batch exactly as the real apply would and is dropped.
        if copy.apply_batch(incoming, &peer, true)?.is_none() {
            return Ok(None);
        }
        let (currency, fx) = self.base_and_fx();
        let (_, copy_fx) = copy.base_and_fx();
        let before = self.store_values_usd();
        let after = copy.store_values_usd();
        let names: HashMap<&str, &str> = self
            .doc
            .stores
            .iter()
            .chain(&copy.doc.stores)
            .map(|store| (store.id.as_str(), store.name.as_str()))
            .collect();
        let mut stores: Vec<StoreBalanceChange> = Vec::new();
        for id in before.keys().chain(after.keys()) {
            if stores.iter().any(|store| &store.id == id) {
                continue;
            }
            let old = before.get(id).copied().unwrap_or_default() * fx;
            let new = after.get(id).copied().unwrap_or_default() * copy_fx;
            if old == new {
                continue;
            }
            stores.push(StoreBalanceChange {
                id: id.clone(),
                name: names
                    .get(id.as_str())
                    .copied()
                    .unwrap_or_default()
                    .to_string(),
                before: old,
                after: new,
                currency: currency.clone(),
            });
        }
        stores.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        Ok(Some(AfterApply {
            currency,
            net_worth_before: self.net_worth().net,
            net_worth_after: copy.net_worth().net,
            stores,
        }))
    }

    /// Merge records from another device and save. `None` when nothing
    /// changed here; otherwise the batch, kept for undo.
    pub fn sync_apply(
        &mut self,
        incoming: &BTreeMap<String, RecordState>,
        peer: &PeerRef,
    ) -> Result<Option<AppliedBatch>> {
        self.apply_batch(incoming, peer, true)
    }

    /// [`Self::sync_apply`]; `carry` off is the earlier dedupe (the copy
    /// made later goes with its edits), which the tests keep as the other
    /// side of a pair of different versions.
    fn apply_batch(
        &mut self,
        incoming: &BTreeMap<String, RecordState>,
        peer: &PeerRef,
        carry: bool,
    ) -> Result<Option<AppliedBatch>> {
        Self::check_incoming(incoming)?;
        self.track_changes()?;
        for state in incoming.values() {
            if let Some(latest) = state.latest() {
                self.clock.observe(latest);
            }
        }
        let mut value = self.doc_value()?;
        let records = view::records(&value);
        // A deletion old enough to be compacted, for a key this device knows
        // nothing about, is not news: the senders had it acked everywhere
        // else. Storing it would only restart its age.
        let incoming = &self.without_compacted(incoming, &records);
        let mut plan = crate::sync::plan::plan(incoming, |key| self.state_of(&records, key));
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
            plan = crate::sync::plan::plan(&rest, |key| self.state_of(&records, key));
            if plan.changes.is_empty() {
                return Ok(None);
            }
            applied_value = value.clone();
            for (key, change) in &plan.changes {
                view::write(&mut applied_value, key, change.after.value.as_ref());
            }
            view::trim(&mut applied_value);
        }
        value = applied_value;
        // Rebuilding the typed document is the schema check: a record that
        // does not fit fails here and nothing is applied.
        let mut doc: VaultDoc = serde_json::from_value(value.clone())
            .map_err(|err| Error::InvalidInput(format!("sync: {err}")))?;
        doc.sync = std::mem::take(&mut self.doc.sync);
        for (key, change) in &plan.changes {
            match &change.after.stamps {
                Some(stamps) => doc.sync.stamps.insert(key.clone(), stamps.clone()),
                None => doc.sync.stamps.remove(key),
            };
            match &change.after.tombstone {
                Some(stamp) => {
                    // Kept from when it arrived, whatever its own stamp says.
                    doc.sync
                        .tombstones_seen
                        .insert(key.clone(), crate::sync::stamp::now_ms());
                    doc.sync.tombstones.insert(key.clone(), stamp.clone())
                }
                None => {
                    doc.sync.tombstones_seen.remove(key);
                    doc.sync.tombstones.remove(key)
                }
            };
        }
        // Log entries the cap cut off keep no stamps.
        let kept = view::records(&value);
        doc.sync.stamps.retain(|key, _| kept.contains_key(key));
        self.doc = doc;
        // A union of two devices' caches can overshoot the caps: thin it the
        // way a refresh does, before the baseline is taken, so the dropped
        // points are neither stamped nor sent as deletions.
        if plan
            .changes
            .keys()
            .any(|key| key.starts_with("price_cache:") || key.starts_with("fx_cache:"))
        {
            self.thin_rate_caches();
        }
        // The typed document may serialize a little differently from the
        // merged JSON (defaults, number forms): the tracker starts from what
        // a save will actually see, or the next save would restamp the batch.
        // Envelopes that arrived whole (made by a device with no journal) get
        // their opening entry; a merged journal's total is re-derived. Both
        // are the same on every device, so they are not stamped as edits.
        self.settle_envelopes();
        let settled = self.doc_value()?;
        if let Some(tracker) = self.tracker.as_mut() {
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
        self.journal_push(
            &applied.peer_name,
            &applied.summary,
            SyncJournalOutcome::Applied,
            Some(applied.id.clone()),
        );
        self.doc.sync.batches.push(batch);
        self.prune_batches();
        self.compact_tombstones();
        // Both devices may have run the same automation or import before
        // they met; the copy made later goes (as an ordinary deletion).
        self.dedupe_generated(&applied.id, carry)?;
        // Things another device deleted may still be named by a record that
        // arrived with the batch (an emergency account, an inflation source).
        // The same pruning runs on every device and the persist below stamps
        // it like a local edit, so it settles to one result without
        // resurrecting anything. Shared events keep their references: a
        // settlement or item that names a removed participant is user data
        // and is left for the invariant pass of stage M.
        self.prune_emergency_fund();
        self.prune_inflation_sources();
        self.recompute_holdings();
        self.persist()?;
        Ok(Some(applied))
    }

    /// `incoming` without the deletions that were already compacted: tombstone
    /// only, older than `TOMBSTONE_DAYS`, for a key with no value and no
    /// tombstone here.
    fn without_compacted(
        &self,
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
                    !records.contains_key(*key) && !self.doc.sync.tombstones.contains_key(*key);
                !(old_deletion && unknown)
            })
            .map(|(key, state)| (key.clone(), state.clone()))
            .collect()
    }

    /// Read another copy of this vault (its sealed bytes) without locking,
    /// writing or syncing anything. Only for reading records out of it.
    fn open_copy(&self, remote: &[u8], password: &str) -> Result<UnlockedVault> {
        let (plaintext, fmt, _key) = crate::crypto::open_bytes_keyed(remote, password.as_bytes())?;
        let decoded = crate::storage::decode_versioned(&plaintext, fmt)?;
        Ok(UnlockedVault {
            path: self.path.clone(),
            fmt: fmt.for_writing(),
            doc: decoded.doc,
            base_currency: self.base_currency.clone(),
            notice_ru: self.notice_ru,
            password: Zeroizing::new(String::new()),
            _lock: crate::storage::VaultLock::detached(),
            // Nothing in the copy is ever written back.
            sandbox: true,
            mirror: None,
            tracker: None,
            clock: self.clock.clone(),
            sync_tap: None,
            announce: std::collections::BTreeSet::new(),
            key: None,
            save: super::saving::SaveState::default(),
        })
    }

    /// Bring the records of `other` (the same vault, another copy) into this
    /// one the way a peer would: what only `other` has or changed later
    /// arrives, what only this vault has stays. Only records travel: `other`'s
    /// batches, journal, peers and pauses are never taken.
    pub fn merge_from(
        &mut self,
        other: &mut UnlockedVault,
        name: &str,
    ) -> Result<Option<AppliedBatch>> {
        if self.doc.id != other.doc.id {
            return Err(Error::InvalidInput(
                "merge: the copy is another vault".into(),
            ));
        }
        let mine = self.sync_manifest(None)?;
        let theirs = other.sync_manifest(None)?;
        let (wanted, _) = self.sync_diff(&mine, &theirs);
        let incoming = other.sync_records(&wanted)?;
        let peer = PeerRef {
            device_id: EXTERNAL_COPY_DEVICE.into(),
            name: name.into(),
        };
        self.sync_apply(&incoming, &peer)
    }

    /// Merge an external copy (sealed bytes, same password) in; the result is
    /// written. `Unmergeable` when it does not open or is another vault.
    pub fn merge_external_copy(
        &mut self,
        remote: &[u8],
        password: &str,
        name: &str,
    ) -> Result<ExternalMerge> {
        let Ok(mut copy) = self.open_copy(remote, password) else {
            return Ok(ExternalMerge::Unmergeable);
        };
        if copy.doc.id != self.doc.id {
            return Ok(ExternalMerge::Unmergeable);
        }
        let applied = self.merge_from(&mut copy, name)?;
        self.persist_now()?;
        Ok(ExternalMerge::Done(applied))
    }

    /// The external copy is the newer one and this vault has no changes of its
    /// own (the file equals the last sync): take its data. The sync
    /// bookkeeping stays this device's own: the copy may come from a device
    /// with other batches, a journal and pauses.
    pub fn take_external_copy(&mut self, remote: &[u8], password: &str) -> Result<ExternalMerge> {
        let Ok(mut copy) = self.open_copy(remote, password) else {
            return Ok(ExternalMerge::Unmergeable);
        };
        if copy.doc.id != self.doc.id {
            return Ok(ExternalMerge::Unmergeable);
        }
        // Stamps and tombstones describe the records, so they come with the
        // data; the rest is this device's own bookkeeping. Trusted peers
        // travel between devices, so they are merged by their own rules.
        let mut sync = std::mem::take(&mut self.doc.sync);
        let remote_sync = std::mem::take(&mut copy.doc.sync);
        sync.take_records_from(remote_sync, crate::sync::stamp::now_ms());
        self.doc = std::mem::take(&mut copy.doc);
        self.doc.sync = sync;
        self.answer_crypto();
        self.start_tracking()?;
        self.ensure_cash_wallet("Cash")?;
        self.ensure_default_categories();
        self.migrate_workflow_graphs();
        self.disable_incomplete_workflows(crate::calendar::now_secs());
        self.prune_attachments();
        self.scrub_sync_secrets();
        self.recompute_holdings();
        self.persist_now()?;
        Ok(ExternalMerge::Done(None))
    }

    /// Keys a kept batch changed (to pass on to the other devices).
    pub fn sync_batch_keys(&self, batch_id: &str) -> Vec<String> {
        self.doc
            .sync
            .batches
            .iter()
            .find(|batch| batch.id == batch_id)
            .map(|batch| {
                batch
                    .entries
                    .iter()
                    .map(|entry| entry.key.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Notices a kept batch added (to show them on this device).
    pub fn sync_batch_notices(&self, batch_id: &str) -> Vec<crate::model::notify::NoticeRecord> {
        let keys: std::collections::HashSet<String> = self
            .sync_batch_keys(batch_id)
            .into_iter()
            .filter_map(|key| key.strip_prefix("notification_log:").map(str::to_string))
            .collect();
        self.doc
            .notification_log
            .iter()
            .filter(|record| keys.contains(&record.key))
            .cloned()
            .collect()
    }

    /// The batches still kept and the transfers to new devices, newest first.
    pub fn sync_history(&self) -> Vec<AppliedBatch> {
        let mut history: Vec<AppliedBatch> = self.doc.sync.batches.iter().map(applied_of).collect();
        history.extend(self.doc.sync.transfers.iter().map(|transfer| AppliedBatch {
            id: transfer.id.clone(),
            peer_name: transfer.device_name.clone(),
            applied_at: transfer.at.clone(),
            summary: SyncSummary::default(),
            undone_at: None,
            transferred_to: Some(transfer.device_name.clone()),
        }));
        history.sort_by_key(|item| {
            std::cmp::Reverse(crate::model::vault::iso_to_millis(&item.applied_at))
        });
        history
    }

    /// The vault was just handed to a new device: trust it for sync, keep it
    /// in the history and leave a notice every linked device shows (so a
    /// transfer the owner did not make does not go unseen). Returns the notice.
    pub fn sync_record_transfer(
        &mut self,
        device_id: &str,
        device_name: &str,
        public_key: &str,
        ru: bool,
    ) -> Result<crate::model::notify::NoticeRecord> {
        let at = now_iso();
        let id = uuid::Uuid::new_v4().to_string();
        self.doc
            .sync
            .transfers
            .push(crate::sync::state::TransferRecord {
                id: id.clone(),
                device_id: device_id.to_string(),
                device_name: device_name.to_string(),
                at: at.clone(),
            });
        self.journal_push(
            device_name,
            &SyncSummary::default(),
            SyncJournalOutcome::Sent,
            None,
        );
        let extra = self
            .doc
            .sync
            .transfers
            .len()
            .saturating_sub(crate::sync::state::MAX_TRANSFERS);
        self.doc.sync.transfers.drain(..extra);
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
        let notice = super::notifications::notice(
            crate::model::notify::NoticeCategory::Security,
            format!("transfer:{id}"),
            &title,
            body,
            Some("/?settings=sync".into()),
            at,
        );
        self.log_notices(vec![notice.clone()])?;
        self.sync_trust_peer(device_id, device_name, public_key)?;
        Ok(notice)
    }

    /// Put back what a batch changed, for every record nobody changed since.
    /// The undo is a new local change: it reaches the other devices too.
    pub fn sync_undo(&mut self, batch_id: &str) -> Result<UndoReport> {
        self.track_changes()?;
        let Some(index) = self
            .doc
            .sync
            .batches
            .iter()
            .position(|batch| batch.id == batch_id)
        else {
            return Err(Error::NotFound(format!("sync batch {batch_id}")));
        };
        if self.doc.sync.batches[index].undone_at.is_some() {
            return Err(Error::InvalidInput("this sync is already undone".into()));
        }
        let entries = self.doc.sync.batches[index].entries.clone();
        let mut value = self.doc_value()?;
        let records = view::records(&value);
        let mut report = UndoReport::default();
        // Log records the batch added: removed with a tombstone, or the peer
        // would send them back.
        let mut forgotten = Vec::new();
        for entry in &entries {
            let current = self.state_of(&records, &entry.key);
            let union = split_key(&entry.key)
                .is_some_and(|(coll, _)| matches!(coll.rule, Rule::Union { .. }));
            if !union && current.same_marks(&entry.after) {
                view::write(&mut value, &entry.key, entry.before.value.as_ref());
                let added_log = entry.before.value.is_none()
                    && split_key(&entry.key)
                        .is_some_and(|(coll, _)| matches!(coll.rule, Rule::Log { .. }));
                if added_log {
                    forgotten.push(entry.key.clone());
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
        doc.sync = std::mem::take(&mut self.doc.sync);
        doc.sync.batches[index].undone_at = Some(now_iso());
        self.doc = doc;
        for key in &forgotten {
            self.sync_forget_log_record(key)?;
        }
        // An undone mark must not make a booked moment fire again.
        self.keep_schedule_marks();
        self.recompute_holdings();
        self.persist()?;
        Ok(report)
    }

    fn prune_batches(&mut self) {
        let cutoff = crate::sync::stamp::now_ms().saturating_sub(day_ms(BATCH_DAYS as u64));
        self.doc.sync.batches.retain(|batch| {
            u64::try_from(crate::model::vault::iso_to_millis(&batch.applied_at)).unwrap_or(0)
                >= cutoff
        });
        let extra = self.doc.sync.batches.len().saturating_sub(MAX_BATCHES);
        self.doc.sync.batches.drain(..extra);
    }

    /// Forget deletions old enough that every device that counts has them.
    /// A deletion counts from the later of its own stamp and the time it
    /// reached this device: one that arrives late is not compacted at once.
    /// Counting devices are the trusted ones and those forgotten less than
    /// `REVOKED_GATE_DAYS` ago; each must have acked the deletion (its manifest
    /// held it, see `sync_ack_tombstones`). No ack yet means keep.
    fn compact_tombstones(&mut self) {
        let now = crate::sync::stamp::now_ms();
        let cutoff = now.saturating_sub(day_ms(TOMBSTONE_DAYS));
        let revoked_cutoff = now.saturating_sub(day_ms(REVOKED_GATE_DAYS));
        let sync = &mut self.doc.sync;
        let acks: Vec<u64> = sync
            .peers
            .iter()
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

    fn created_of(&self, key: &str) -> Stamp {
        self.doc
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
    fn dedupe_generated(&mut self, batch_id: &str, carry: bool) -> Result<()> {
        struct Winner {
            rank: (Stamp, String),
            run_id: Option<String>,
        }
        fn natural_key(origin: &TxOrigin) -> Option<(String, Option<&str>)> {
            match origin {
                TxOrigin::Import { exec_id } => Some((format!("import:{exec_id}"), None)),
                // An automation is a workflow now and kept its id: both
                // kinds of origin name the same planned moment alike.
                TxOrigin::Automation {
                    automation_id: maker,
                    planned_at,
                } => Some((format!("auto:{maker}:{planned_at}"), None)),
                // The planned moment first: an old peer's copy of the same
                // moment has no key, and both must meet.
                TxOrigin::Workflow {
                    workflow_id: maker,
                    run_id,
                    planned_at: Some(planned_at),
                    ..
                } => Some((format!("auto:{maker}:{planned_at}"), Some(run_id))),
                TxOrigin::Workflow {
                    run_id,
                    run_key: Some(run_key),
                    ..
                } => Some((format!("run:{run_key}"), Some(run_id))),
                _ => None,
            }
        }

        // (key of the record that goes, key of the record it passes edits to)
        let mut removals: Vec<(String, Option<String>)> = Vec::new();

        let mut best: HashMap<String, Winner> = HashMap::new();
        for tx in &self.doc.transactions {
            let meta = tx.meta();
            let Some((natural, run)) = natural_key(&meta.origin) else {
                continue;
            };
            let rank = (
                self.created_of(&format!("transactions:{}", meta.id)),
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
        for tx in &self.doc.transactions {
            let meta = tx.meta();
            let Some((natural, run)) = natural_key(&meta.origin) else {
                continue;
            };
            let Some(winner) = best.get(&natural) else {
                continue;
            };
            if winner.rank.1 == meta.id {
                continue;
            }
            let stamp = self.created_of(&format!("transactions:{}", meta.id));
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

        // An automatic reminder is one per source (kind + entity id), however
        // its id or its key was made. Reminders of the old build (random id,
        // schedule-only key) group with the new ones by their source.
        fn identity(reminder: &Reminder) -> Option<String> {
            match &reminder.source {
                Some(source) => Some(source_identity(source)),
                None => (!reminder.source_key.is_empty()).then(|| reminder.source_key.clone()),
            }
        }
        let mut first: HashMap<String, (Stamp, String)> = HashMap::new();
        for reminder in &self.doc.reminders {
            let Some(group) = identity(reminder) else {
                continue;
            };
            let rank = (
                self.created_of(&format!("reminders:{}", reminder.id)),
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
        for reminder in &self.doc.reminders {
            if let Some(group) = identity(reminder)
                && let Some((_, kept)) = first.get(&group)
                && *kept != reminder.id
            {
                removals.push((
                    format!("reminders:{}", reminder.id),
                    Some(format!("reminders:{kept}")),
                ));
            }
        }
        self.remove_duplicates(removals, batch_id, carry)
    }

    /// Delete the records of `removals` as ordinary deletions (a tombstone
    /// each), after moving their later-edited fields into the record named
    /// beside them. Everything is recorded in the batch for undo and offered
    /// to a live session; the change tracker is rebased so none of it is
    /// stamped a second time.
    fn remove_duplicates(
        &mut self,
        removals: Vec<(String, Option<String>)>,
        batch_id: &str,
        carry: bool,
    ) -> Result<()> {
        if removals.is_empty() {
            return Ok(());
        }
        let mut value = self.doc_value()?;
        let records = view::records(&value);
        let mut sync = self.doc.sync.clone();
        let mut touched: BTreeMap<String, RecordState> = BTreeMap::new();
        let mut after: BTreeMap<String, RecordState> = BTreeMap::new();
        for (loser, into) in removals {
            touched
                .entry(loser.clone())
                .or_insert_with(|| self.state_of(&records, &loser));
            if carry
                && let Some(winner) = &into
                && let Some((coll, _)) = split_key(winner)
                && let (Some(from), Some(to)) =
                    (view::read(&value, &loser), view::read(&value, winner))
            {
                touched
                    .entry(winner.clone())
                    .or_insert_with(|| self.state_of(&records, winner));
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
                        self.announce.insert(winner.clone());
                    }
                }
            }
            view::write(&mut value, &loser, None);
            sync.stamps.remove(&loser);
            let deleted = self.clock.tick();
            sync.tombstones.insert(loser.clone(), deleted.clone());
            after.insert(
                loser.clone(),
                RecordState {
                    value: None,
                    stamps: None,
                    tombstone: Some(deleted),
                },
            );
            self.announce.insert(loser);
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
        self.doc = doc;
        let settled = self.doc_value()?;
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.rebase(&settled);
        }
        Ok(())
    }

    // -- trusted devices ------------------------------------------------------

    /// Trust a device after both sides confirmed the pairing code.
    pub fn sync_trust_peer(&mut self, device_id: &str, name: &str, public_key: &str) -> Result<()> {
        // The new entry must sort after whatever is known of this device/key.
        for known in &self.doc.sync.peers {
            if known.device_id == device_id || known.public_key == public_key {
                self.clock.observe(&known.stamp);
            }
        }
        let stamp = self.clock.tick();
        let peer = TrustedPeer {
            device_id: device_id.to_string(),
            name: name.to_string(),
            public_key: public_key.to_string(),
            paired_at: now_iso(),
            last_synced_at: None,
            revoked: None,
            stamp,
        };
        let peers = &mut self.doc.sync.peers;
        match peers.iter_mut().find(|known| known.device_id == device_id) {
            Some(known) => *known = peer,
            None => peers.push(peer),
        }
        // Paired afresh: whatever it told us before no longer holds.
        self.doc.sync.cut_by.remove(device_id);
        self.doc.sync.paused.remove(device_id);
        self.doc.sync.tombstone_acks.remove(device_id);
        self.persist()
    }

    /// A vault file that arrived by transfer carries the sender's sync
    /// bookkeeping verbatim. Keep the record metadata and the trusted devices,
    /// drop everything device-local (undo batches, journal, transfers,
    /// pauses, cut-offs, deletion arrival times) and the peers' last-sync times.
    pub fn adopt_received_copy(&mut self) -> Result<()> {
        let received = std::mem::take(&mut self.doc.sync);
        let mut fresh = crate::sync::SyncState::default();
        fresh.take_records_from(received, crate::sync::stamp::now_ms());
        for peer in &mut fresh.peers {
            peer.last_synced_at = None;
        }
        self.doc.sync = fresh;
        self.persist()
    }

    /// "Forget device": its key is refused here and, once this reaches
    /// them, on every other device.
    pub fn sync_forget_peer(&mut self, device_id: &str) -> Result<()> {
        if let Some(known) = self
            .doc
            .sync
            .peers
            .iter()
            .find(|peer| peer.device_id == device_id)
        {
            self.clock.observe(&known.stamp);
        }
        let stamp = self.clock.tick();
        let Some(peer) = self
            .doc
            .sync
            .peers
            .iter_mut()
            .find(|peer| peer.device_id == device_id)
        else {
            return Err(Error::NotFound(format!("device {device_id}")));
        };
        peer.revoked = Some(stamp.clone());
        peer.stamp = stamp;
        // A forgotten device pairs afresh; an old pause must not block that.
        self.doc.sync.paused.remove(device_id);
        self.doc.sync.cut_by.remove(device_id);
        self.persist()
    }

    /// Take in another device's list of trusted devices.
    pub fn sync_merge_peers(&mut self, remote: &[TrustedPeer]) -> Result<bool> {
        let now = crate::sync::stamp::now_ms();
        for peer in remote {
            self.clock.observe_at(&peer.stamp, now);
        }
        let changed = self.doc.sync.merge_peers(remote, now);
        if changed {
            self.persist()?;
        }
        Ok(changed)
    }

    /// Pause exchange with a device (the user declined its list of changes).
    /// Kept in this vault only; it is never sent to other devices.
    pub fn sync_pause_peer(&mut self, device_id: &str) -> Result<()> {
        if self.doc.sync.paused.insert(device_id.to_string()) {
            self.persist()?;
        }
        Ok(())
    }

    /// "Resume": exchange with the device may run again.
    pub fn sync_resume_peer(&mut self, device_id: &str) -> Result<()> {
        if self.doc.sync.paused.remove(device_id) {
            self.persist()?;
        }
        Ok(())
    }

    pub fn sync_is_paused(&self, device_id: &str) -> bool {
        self.doc.sync.paused.contains(device_id)
    }

    /// The device told us it cut this one off (`reason`). Returns whether this
    /// is news (a new device or another reason); only news is saved.
    pub fn sync_cut_by(&mut self, device_id: &str, reason: SyncCutReason) -> Result<bool> {
        if self
            .doc
            .sync
            .cut_by
            .get(device_id)
            .is_some_and(|known| known.reason == reason)
        {
            return Ok(false);
        }
        self.doc.sync.cut_by.insert(
            device_id.to_string(),
            CutBy {
                reason,
                at: now_iso(),
            },
        );
        self.persist()?;
        Ok(true)
    }

    /// The way back was taken (an exchange, a pairing): the device may be
    /// dialed again. Returns whether there was anything to clear.
    pub fn sync_clear_cut(&mut self, device_id: &str) -> Result<bool> {
        if self.doc.sync.cut_by.remove(device_id).is_none() {
            return Ok(false);
        }
        self.persist()?;
        Ok(true)
    }

    pub fn sync_cut_reason(&self, device_id: &str) -> Option<SyncCutReason> {
        self.doc.sync.cut_by.get(device_id).map(|cut| cut.reason)
    }

    /// Record what the manifest `theirs` of `device_id` vouches for: how far
    /// our deletions are all in it. Call it only for an exchange that was
    /// applied: a declined or failed one proves nothing. Never goes down.
    pub fn sync_ack_tombstones(&mut self, device_id: &str, theirs: &Manifest) -> Result<()> {
        let ours = self.sync_manifest(None)?;
        let sync = &self.doc.sync;
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
            self.doc
                .sync
                .tombstone_acks
                .insert(device_id.to_string(), acked);
            self.persist()?;
        }
        Ok(())
    }

    /// Note a finished exchange with a device (not synced as a change).
    pub fn sync_mark_synced(&mut self, device_id: &str) -> Result<()> {
        if let Some(peer) = self
            .doc
            .sync
            .peers
            .iter_mut()
            .find(|peer| peer.device_id == device_id)
        {
            peer.last_synced_at = Some(now_iso());
        }
        self.persist()
    }

    // -- backups --------------------------------------------------------------

    fn presync_path(&self, index: usize) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        if index == 0 {
            name.push(".presync.bak");
        } else {
            name.push(format!(".presync.{index}.bak"));
        }
        self.path.with_file_name(name)
    }

    /// Copy the vault file aside before the first batch of a session,
    /// keeping the last [`PRESYNC_BACKUPS`].
    pub fn sync_backup(&mut self) -> Result<PathBuf> {
        if self.sandbox {
            return Ok(self.presync_path(0));
        }
        for index in (1..PRESYNC_BACKUPS).rev() {
            let from = self.presync_path(index - 1);
            if from.exists() {
                std::fs::rename(&from, self.presync_path(index))
                    .map_err(|err| Error::Io(err.to_string()))?;
            }
        }
        let target = self.presync_path(0);
        // The copy is of the file: batched changes go to it first.
        self.flush_pending()?;
        std::fs::copy(&self.path, &target).map_err(|err| Error::Io(err.to_string()))?;
        Ok(target)
    }

    /// Delete every pre-sync backup (a missing file is fine).
    pub(super) fn remove_sync_backups(&self) {
        for index in 0..PRESYNC_BACKUPS {
            let _ = std::fs::remove_file(self.presync_path(index));
        }
    }

    /// Whether a pre-sync backup exists.
    pub fn has_sync_backup(&self) -> bool {
        self.presync_path(0).exists()
    }

    /// The emergency exit: the whole vault as it was before the last
    /// session's first batch. Saved as new local changes, so it reaches the
    /// other devices like any edit.
    pub fn sync_restore_backup(&mut self) -> Result<()> {
        let path = self.presync_path(0);
        if !path.exists() {
            return Err(Error::NotFound("no backup from before a sync".into()));
        }
        let (doc, _) = VaultFile::open(&path, &self.password)?;
        self.track_changes()?;
        self.replace_doc(doc)
    }
}

fn day_ms(days: u64) -> u64 {
    days.saturating_mul(24 * 3600 * 1000)
}

fn applied_of(batch: &SyncBatch) -> AppliedBatch {
    AppliedBatch {
        id: batch.id.clone(),
        peer_name: batch.peer_name.clone(),
        applied_at: batch.applied_at.clone(),
        summary: batch.summary.clone(),
        undone_at: batch.undone_at.clone(),
        transferred_to: None,
    }
}

#[cfg(test)]
mod adjustment_tests;
#[cfg(test)]
mod clock_tests;
#[cfg(test)]
mod convergence_catalog;
#[cfg(test)]
mod convergence_tests;
#[cfg(test)]
mod envelope_tests;
#[cfg(test)]
mod log_tests;
#[cfg(test)]
mod marks_tests;
#[cfg(test)]
mod nested_tests;
#[cfg(test)]
mod preview_tests;
#[cfg(test)]
mod reminder_tests;
#[cfg(test)]
mod run_key_tests;
#[cfg(test)]
mod terminal_tests;
#[cfg(test)]
mod tombstone_tests;
#[cfg(test)]
mod transfer_tests;

#[cfg(test)]
mod tests {
    use super::super::test_support::{empty_vault, income};
    use super::*;
    use crate::model::transaction::Transaction;
    use crate::sync::manifest::diff;

    /// The same vault on two devices: a copy of one file, opened twice.
    pub(super) fn two_devices() -> (tempfile::TempDir, UnlockedVault, UnlockedVault) {
        let (dir, mut left) = empty_vault();
        left.set_device("aaaaaaaa-left");
        left.persist().unwrap();
        let copy = dir.path().join("copy.wallet");
        std::fs::copy(&left.path, &copy).unwrap();
        let mut right = UnlockedVault::open(copy, "pw").unwrap();
        right.set_device("bbbbbbbb-right");
        (dir, left, right)
    }

    fn peer(name: &str) -> PeerRef {
        PeerRef {
            device_id: format!("{name}-id"),
            name: name.into(),
        }
    }

    /// One exchange as a session runs it; returns what each side applied.
    pub(super) fn exchange(
        left: &mut UnlockedVault,
        right: &mut UnlockedVault,
    ) -> (Option<AppliedBatch>, Option<AppliedBatch>) {
        let left_manifest = left.sync_manifest(None).unwrap();
        let right_manifest = right.sync_manifest(None).unwrap();
        let (left_wants, _) = left.sync_diff(&left_manifest, &right_manifest);
        let (right_wants, _) = right.sync_diff(&right_manifest, &left_manifest);
        let to_left = right.sync_records(&left_wants).unwrap();
        let to_right = left.sync_records(&right_wants).unwrap();
        let applied_left = left.sync_apply(&to_left, &peer("right")).unwrap();
        let applied_right = right.sync_apply(&to_right, &peer("left")).unwrap();
        (applied_left, applied_right)
    }

    /// Exchange until nothing moves; both sides must then agree.
    pub(super) fn settle(left: &mut UnlockedVault, right: &mut UnlockedVault) {
        for _ in 0..4 {
            if exchange(left, right) == (None, None) {
                break;
            }
        }
        assert_eq!(
            left.sync_manifest(None).unwrap(),
            right.sync_manifest(None).unwrap(),
            "devices did not converge"
        );
    }

    struct NoWake;
    impl super::super::saving::SaveHook for NoWake {
        fn dirty(&self) {}
    }

    /// Collects what the vault announces, in place of a live link.
    #[derive(Default)]
    struct Announced(std::sync::Mutex<Vec<String>>);

    impl SyncTap for Announced {
        fn changed(&self, keys: &[String]) {
            self.0.lock().unwrap().extend(keys.iter().cloned());
        }
    }

    #[test]
    fn an_edit_just_before_a_sync_call_is_pushed_in_the_same_session() {
        let (_dir, mut left, right) = two_devices();
        let tap = Arc::new(Announced::default());
        left.set_sync_tap(Some(tap.clone()));
        left.set_write_behind(Some(Arc::new(NoWake)));
        left.start_tracking().unwrap();
        let cash = cash_id(&left);
        left.doc.stores[0].name = "Renamed in a hurry".into();
        left.persist().unwrap();
        // A sync call stamps the pending edit before the save-time push.
        left.sync_manifest(None).unwrap();
        let job = left.prepare_flush().unwrap().expect("dirty");
        drop(job);
        let sent = tap.0.lock().unwrap().clone();
        assert!(
            sent.iter().any(|key| key.contains(&cash)),
            "the edit was stamped by the sync call and never announced: {sent:?}"
        );
        drop(right);
    }

    fn fx_point(currency: &str, ts: &str) -> crate::model::vault::FxPoint {
        crate::model::vault::FxPoint {
            currency: currency.into(),
            per_usd: crate::Amount::from(2u32),
            ts: ts.into(),
            source: "test".into(),
        }
    }

    /// How many cache records the next exchange would move, both ways.
    fn cache_records_moving(left: &mut UnlockedVault, right: &mut UnlockedVault) -> usize {
        let left_manifest = left.sync_manifest(None).unwrap();
        let right_manifest = right.sync_manifest(None).unwrap();
        let (left_wants, _) = left.sync_diff(&left_manifest, &right_manifest);
        let (right_wants, _) = right.sync_diff(&right_manifest, &left_manifest);
        left_wants
            .iter()
            .chain(&right_wants)
            .filter(|key| key.starts_with("fx_cache:") || key.starts_with("price_cache:"))
            .count()
    }

    #[test]
    fn an_fx_refresh_on_each_copy_asks_nothing() {
        let (_dir, mut left, mut right) = two_devices();
        let currencies = ["chf", "jpy", "pln"];
        left.apply_fx(&currencies.map(|code| fx_point(code, "2026-11-03T10:00:00Z")));
        right.apply_fx(&currencies.map(|code| fx_point(code, "2026-11-03T12:00:00Z")));
        // Whatever an older peer would still offer, point by point.
        let keys: Vec<String> = left
            .doc
            .fx_cache
            .iter()
            .map(|point| format!("fx_cache:{}|{}", point.currency, point.ts))
            .collect();
        assert!(!keys.is_empty());
        let incoming = left.sync_records(&keys).unwrap();
        let summary = right.sync_preview(&incoming).unwrap();
        assert_eq!(
            summary.total, 0,
            "cache points reached the question: {summary:?}"
        );
        assert!(summary.collections.is_empty(), "{summary:?}");
    }

    #[test]
    fn thinned_cache_points_are_not_requested_again() {
        let (_dir, mut left, mut right) = two_devices();
        // The base currency is a device setting: only the left copy keeps
        // euro history; the right one thins whatever arrives to one point.
        left.base_currency = "eur".into();
        for hour in ["08", "10", "12"] {
            left.apply_fx(&[fx_point("eur", &format!("2026-11-03T{hour}:00:00Z"))]);
        }
        exchange(&mut left, &mut right);
        exchange(&mut left, &mut right);
        assert_eq!(cache_records_moving(&mut left, &mut right), 0);
    }

    #[test]
    fn a_cache_point_of_a_currency_in_use_still_syncs() {
        let (_dir, mut left, mut right) = two_devices();
        for vault in [&mut left, &mut right] {
            vault.doc.settings.budget_currency = "eur".into();
            vault.persist().unwrap();
        }
        left.apply_fx(&[fx_point("eur", "2026-11-03T10:00:00Z")]);
        right.apply_fx(&[fx_point("eur", "2026-11-03T12:00:00Z")]);
        settle(&mut left, &mut right);
        for vault in [&left, &right] {
            let stamps: Vec<&str> = vault
                .doc
                .fx_cache
                .iter()
                .filter(|point| point.currency == "eur")
                .map(|point| point.ts.as_str())
                .collect();
            assert!(stamps.contains(&"2026-11-03T10:00:00Z"), "{stamps:?}");
            assert!(stamps.contains(&"2026-11-03T12:00:00Z"), "{stamps:?}");
        }
        assert_eq!(cache_records_moving(&mut left, &mut right), 0);
    }

    fn snapshot(
        month: &str,
        color: crate::model::planning::HealthColor,
        reasons: Vec<crate::model::planning::HealthReasonCode>,
    ) -> Option<crate::model::planning::HealthSnapshot> {
        Some(crate::model::planning::HealthSnapshot {
            month: month.into(),
            color,
            reasons,
        })
    }

    #[test]
    fn health_last_with_an_empty_slot_converges() {
        use crate::model::planning::{HealthColor, HealthReasonCode};
        let (_dir, mut left, mut right) = two_devices();
        left.doc.health_last = snapshot(
            "2026-10",
            HealthColor::Yellow,
            vec![HealthReasonCode::NoHistory],
        );
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(right.doc.health_last, left.doc.health_last);
        assert!(right.doc.health_last.is_some());

        // Emptying the slot travels like any other edit.
        left.doc.health_last = None;
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(right.doc.health_last, None);
        assert_eq!(left.doc.health_last, None);
    }

    #[test]
    fn two_health_snapshots_do_not_mix_fields() {
        use crate::model::planning::{HealthColor, HealthReasonCode};
        let (_dir, mut left, mut right) = two_devices();
        let base = snapshot("2026-09", HealthColor::Green, vec![]);
        left.doc.health_last = base.clone();
        left.persist().unwrap();
        settle(&mut left, &mut right);
        // The right device writes a whole snapshot first ...
        let earlier = snapshot(
            "2026-10",
            HealthColor::Red,
            vec![HealthReasonCode::NoHistory],
        );
        right.doc.health_last = earlier;
        right.persist().unwrap();
        // ... the left one a later snapshot that keeps the colour of the base.
        let later = snapshot(
            "2026-10",
            HealthColor::Green,
            vec![HealthReasonCode::CostlyLoan],
        );
        left.doc.health_last = later.clone();
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(left.doc.health_last, later, "the later snapshot wins whole");
        assert_eq!(right.doc.health_last, later);
    }

    #[test]
    fn a_repeated_session_with_equal_health_makes_no_batch() {
        use crate::model::planning::{HealthColor, HealthReasonCode};
        let (_dir, mut left, mut right) = two_devices();
        // Both empty.
        settle(&mut left, &mut right);
        assert_eq!(exchange(&mut left, &mut right), (None, None));
        // Both equal after a sync.
        left.doc.health_last = snapshot(
            "2026-10",
            HealthColor::Red,
            vec![HealthReasonCode::NoHistory],
        );
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(exchange(&mut left, &mut right), (None, None));
        assert_eq!(exchange(&mut left, &mut right), (None, None));
    }

    fn cash_id(vault: &UnlockedVault) -> String {
        vault.doc.stores[0].id.clone()
    }

    fn comments(vault: &UnlockedVault) -> Vec<String> {
        let mut out: Vec<String> = vault
            .doc
            .transactions
            .iter()
            .map(|tx| tx.meta().comment.clone())
            .collect();
        out.sort();
        out
    }

    /// `count` hourly BTC quotes from 2025-01-01, `offset` seconds late.
    fn hourly_prices(count: i64, offset: i64) -> Vec<crate::model::vault::PricePoint> {
        use time::format_description::well_known::Rfc3339;
        let base = 1_735_689_600;
        (0..count)
            .map(|hour| crate::model::vault::PricePoint {
                asset_key: "crypto:btc".into(),
                value_usd: crate::Amount::from(u32::try_from(hour).unwrap() + 1),
                ts: time::OffsetDateTime::from_unix_timestamp(base + hour * 3600 + offset)
                    .unwrap()
                    .format(&Rfc3339)
                    .unwrap(),
                source: "test".into(),
                change_24h_pct: None,
            })
            .collect()
    }

    fn price_keys(vault: &UnlockedVault) -> Vec<String> {
        let mut keys: Vec<String> = vault
            .doc
            .price_cache
            .iter()
            .map(|point| format!("{}@{}", point.asset_key, point.ts))
            .collect();
        keys.sort();
        keys
    }

    fn set_prices(vault: &mut UnlockedVault, points: Vec<crate::model::vault::PricePoint>) {
        vault.doc.price_cache = points;
        vault.persist().unwrap();
    }

    fn within_caps(vault: &UnlockedVault) -> bool {
        use crate::pricing::history::{HISTORY_CAP, HOURLY_KEEP};
        vault.doc.price_cache.len() <= HISTORY_CAP
            && vault.doc.price_cache.len() < 2 * HOURLY_KEEP + 400
    }

    fn file_bytes(vault: &UnlockedVault) -> Vec<u8> {
        std::fs::read(&vault.path).unwrap()
    }

    const COPY: &str = "Внешняя копия";

    #[test]
    fn merge_keeps_an_edit_made_only_locally_and_one_made_only_remotely() {
        // A edits X and locks; C saves without X; A opens: X must survive.
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        left.add_transaction(income(&cash, "10", "X")).unwrap();
        right.add_transaction(income(&cash, "20", "Y")).unwrap();

        let outcome = left
            .merge_external_copy(&file_bytes(&right), "pw", COPY)
            .unwrap();
        let ExternalMerge::Done(Some(batch)) = outcome else {
            panic!("expected an applied batch, got {outcome:?}");
        };
        assert_eq!(batch.peer_name, COPY);
        assert_eq!(comments(&left), ["X", "Y"]);
        // What a push would now send holds both.
        let reopened = UnlockedVault::open(left.path.clone(), "pw").unwrap();
        assert_eq!(comments(&reopened), ["X", "Y"]);
    }

    #[test]
    fn merge_refuses_another_vault() {
        let (_dir, mut left, _right) = two_devices();
        let (_other_dir, other) = empty_vault();
        assert_ne!(left.doc.id, other.doc.id);
        let mut other = other;
        assert!(left.merge_from(&mut other, COPY).is_err());
        assert_eq!(
            left.merge_external_copy(&file_bytes(&other), "pw", COPY)
                .unwrap(),
            ExternalMerge::Unmergeable
        );
    }

    #[test]
    fn merge_keeps_the_local_bookkeeping_and_leaves_the_remote_one() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        left.add_transaction(income(&cash, "10", "mine")).unwrap();
        right
            .add_transaction(income(&cash, "20", "theirs"))
            .unwrap();
        // The remote carries bookkeeping of its own.
        right.doc.sync.paused.insert("foreign-device".into());
        right
            .doc
            .sync
            .tombstones_seen
            .insert("transactions:foreign".into(), 1);
        right.persist().unwrap();
        let remote_batches = right.doc.sync.batches.clone();
        let remote_journal = right.doc.sync.journal.clone();

        let batches_before: Vec<String> = left
            .doc
            .sync
            .batches
            .iter()
            .map(|batch| batch.id.clone())
            .collect();
        let journal_before = left.doc.sync.journal.len();
        left.merge_external_copy(&file_bytes(&right), "pw", COPY)
            .unwrap();

        let batches_after: Vec<String> = left
            .doc
            .sync
            .batches
            .iter()
            .map(|batch| batch.id.clone())
            .collect();
        assert_eq!(batches_after[..batches_before.len()], batches_before[..]);
        // Only the merge's own batch was added.
        assert_eq!(batches_after.len(), batches_before.len() + 1);
        assert!(
            remote_batches
                .iter()
                .all(|batch| !batches_after.contains(&batch.id))
        );
        assert_eq!(left.doc.sync.journal.len(), journal_before + 1);
        assert!(
            remote_journal
                .iter()
                .all(|entry| !left.doc.sync.journal.contains(entry))
        );
        assert!(!left.doc.sync.paused.contains("foreign-device"));
        assert!(
            !left
                .doc
                .sync
                .tombstones_seen
                .contains_key("transactions:foreign")
        );
    }

    #[test]
    fn merge_with_a_wrong_password_is_unmergeable() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        right
            .add_transaction(income(&cash, "20", "theirs"))
            .unwrap();
        assert_eq!(
            left.merge_external_copy(&file_bytes(&right), "not-the-password", COPY)
                .unwrap(),
            ExternalMerge::Unmergeable
        );
        assert!(comments(&left).is_empty());
    }

    #[test]
    fn a_pull_from_a_copy_with_a_foreign_sync_state_keeps_the_local_one() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        right
            .add_transaction(income(&cash, "20", "theirs"))
            .unwrap();
        right.doc.sync.paused.insert("foreign-device".into());
        right
            .sync_trust_peer("foreign-id", "Foreign", "key")
            .unwrap();
        left.doc.sync.paused.insert("local-device".into());
        left.persist().unwrap();
        let local_sync = left.doc.sync.clone();

        let outcome = left.take_external_copy(&file_bytes(&right), "pw").unwrap();
        assert_eq!(outcome, ExternalMerge::Done(None));
        assert_eq!(comments(&left), ["theirs"]);
        assert_eq!(left.doc.sync.paused, local_sync.paused);
        // Trusted peers travel between devices: the copy's join the local ones.
        assert!(
            local_sync
                .peers
                .iter()
                .all(|peer| left.doc.sync.peers.contains(peer))
        );
        assert!(
            left.doc
                .sync
                .peers
                .iter()
                .any(|peer| peer.device_id == "foreign-id")
        );
        assert_eq!(left.doc.sync.batches, local_sync.batches);
        assert_eq!(left.doc.sync.journal, local_sync.journal);
        let reopened = UnlockedVault::open(left.path.clone(), "pw").unwrap();
        assert_eq!(comments(&reopened), ["theirs"]);
        assert!(!reopened.doc.sync.paused.contains("foreign-device"));
    }

    fn tx_key(vault: &mut UnlockedVault) -> String {
        vault
            .sync_manifest(None)
            .unwrap()
            .keys()
            .find(|key| key.starts_with("transactions:"))
            .cloned()
            .expect("a transaction record")
    }

    fn set_comment(vault: &mut UnlockedVault, comment: &str) {
        let mut tx = vault.doc.transactions[0].clone();
        let id = tx.meta().id.clone();
        tx.meta_mut().comment = comment.into();
        vault.update_transaction(&id, tx).unwrap();
    }

    /// One shared transaction on both sides.
    fn shared_transaction() -> (tempfile::TempDir, UnlockedVault, UnlockedVault, String) {
        let (dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        left.add_transaction(income(&cash, "10", "shared")).unwrap();
        settle(&mut left, &mut right);
        let key = tx_key(&mut left);
        (dir, left, right, key)
    }

    #[test]
    fn a_pulled_edit_keeps_its_remote_stamp() {
        let (_dir, mut left, mut right, key) = shared_transaction();
        // A peer's edit, older than the one the copy holds.
        set_comment(&mut right, "peer edit");
        let older = right.sync_records(std::slice::from_ref(&key)).unwrap();
        set_comment(&mut right, "copy edit");
        let remote_stamps = right.doc.sync.stamps[&key].clone();

        left.take_external_copy(&file_bytes(&right), "pw").unwrap();
        assert_eq!(comments(&left), ["copy edit"]);
        assert_eq!(left.doc.sync.stamps[&key], remote_stamps);
        // The older edit must not win over the pulled value.
        assert_eq!(left.sync_apply(&older, &peer("third")).unwrap(), None);
        assert_eq!(comments(&left), ["copy edit"]);
    }

    #[test]
    fn a_deletion_in_the_pulled_copy_stays_deleted() {
        let (_dir, mut left, mut right, key) = shared_transaction();
        let alive = left.sync_records(std::slice::from_ref(&key)).unwrap();
        let id = right.doc.transactions[0].meta().id.clone();
        right.delete_transaction(&id).unwrap();

        left.take_external_copy(&file_bytes(&right), "pw").unwrap();
        assert!(left.doc.transactions.is_empty());
        assert!(left.doc.sync.tombstones.contains_key(&key));
        // A peer that still offers the old record does not bring it back.
        assert_eq!(left.sync_apply(&alive, &peer("third")).unwrap(), None);
        assert!(left.doc.transactions.is_empty());
    }

    #[test]
    fn a_pull_from_another_vault_is_unmergeable() {
        let (_dir, mut left, _right) = two_devices();
        let (_other_dir, other) = empty_vault();
        assert_eq!(
            left.take_external_copy(&file_bytes(&other), "pw").unwrap(),
            ExternalMerge::Unmergeable
        );
    }

    #[test]
    fn caches_thinned_apart_converge_to_one_thinned_set() {
        use crate::pricing::history::thin_prices;
        let (_dir, mut left, mut right) = two_devices();
        let mut own_left = hourly_prices(500, 0);
        let mut own_right = hourly_prices(500, 1800);
        thin_prices(&mut own_left);
        thin_prices(&mut own_right);
        set_prices(&mut left, own_left);
        set_prices(&mut right, own_right);

        exchange(&mut left, &mut right);
        // The union overshoots a single device's thinning; both cut it back.
        let mut expected = left.doc.price_cache.clone();
        thin_prices(&mut expected);
        assert_eq!(left.doc.price_cache.len(), expected.len());
        assert!(within_caps(&left));
        assert_eq!(price_keys(&left), price_keys(&right));

        // A second diff of the price collections is empty.
        let left_manifest = left.sync_manifest(None).unwrap();
        let right_manifest = right.sync_manifest(None).unwrap();
        let (left_wants, right_wants) = diff(&left_manifest, &right_manifest);
        assert!(left_wants.is_empty() && right_wants.is_empty());
        settle(&mut left, &mut right);
    }

    #[test]
    fn a_peer_that_never_thins_does_not_grow_this_side() {
        let (_dir, mut left, mut right) = two_devices();
        let dense = hourly_prices(800, 0);
        let mut seen = None;
        for _ in 0..4 {
            // The old peer still holds (and offers) every point it ever had.
            set_prices(&mut right, dense.clone());
            exchange(&mut left, &mut right);
            assert!(within_caps(&left));
            let len = left.doc.price_cache.len();
            assert!(len < dense.len(), "left kept {len} of {}", dense.len());
            if let Some(before) = seen {
                assert_eq!(before, len, "the cache grew between exchanges");
            }
            seen = Some(len);
        }
    }

    #[test]
    fn undo_of_a_batch_with_thinned_away_points_does_not_panic() {
        let (_dir, mut left, mut right) = two_devices();
        set_prices(&mut right, hourly_prices(800, 0));
        let (applied, _) = exchange(&mut left, &mut right);
        let batch = applied.expect("left received prices");
        assert!(left.doc.price_cache.len() < 800);
        let report = left.sync_undo(&batch.id).unwrap();
        // Cached points are a union: it has no deletion, so undo lists them
        // as skipped instead of removing what a peer would send back.
        assert_eq!(report.restored, 0);
        assert!(!report.skipped.is_empty());
        assert!(left.doc.price_cache.len() <= 800);
        // The next thinning cuts anything the undo brought back.
        let mut again = left.doc.price_cache.clone();
        crate::pricing::history::thin_prices(&mut again);
        assert!(again.len() <= left.doc.price_cache.len());
    }

    #[test]
    fn edits_on_both_devices_end_up_on_both() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        left.add_transaction(income(&cash, "10", "from left"))
            .unwrap();
        right
            .add_transaction(income(&cash, "20", "from right"))
            .unwrap();

        let (applied, _) = exchange(&mut left, &mut right);
        let summary = applied.expect("left took something").summary;
        assert_eq!(summary.total, 1);
        assert_eq!(summary.collections[0].collection, "transactions");
        settle(&mut left, &mut right);

        assert_eq!(comments(&left), vec!["from left", "from right"]);
        assert_eq!(comments(&right), comments(&left));
        // Holdings are re-derived, not synced.
        assert_eq!(left.doc.holdings.len(), right.doc.holdings.len());
    }

    #[test]
    fn different_fields_of_one_record_both_survive() {
        let (_dir, mut left, mut right) = two_devices();
        left.doc.stores[0].name = "Wallet".into();
        left.persist().unwrap();
        right.doc.stores[0].color = "#123456".into();
        right.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(left.doc.stores[0].name, "Wallet");
        assert_eq!(left.doc.stores[0].color, "#123456");
        assert_eq!(right.doc.stores[0], left.doc.stores[0]);
    }

    #[test]
    fn the_later_edit_of_one_field_wins_and_is_reported() {
        let (_dir, mut left, mut right) = two_devices();
        left.doc.stores[0].name = "Older".into();
        left.persist().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        right.doc.stores[0].name = "Newer".into();
        right.persist().unwrap();
        let (applied, _) = exchange(&mut left, &mut right);
        let summary = applied.expect("left changed").summary;
        assert_eq!(summary.overwritten.len(), 1);
        assert_eq!(summary.overwritten[0].before, "Older");
        assert_eq!(summary.overwritten[0].after, "Newer");
        settle(&mut left, &mut right);
        assert_eq!(right.doc.stores[0].name, "Newer");
    }

    #[test]
    fn a_deletion_reaches_the_other_device() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        let tx = left.add_transaction(income(&cash, "10", "doomed")).unwrap();
        settle(&mut left, &mut right);
        assert_eq!(comments(&right), vec!["doomed"]);
        right.delete_transaction(&tx.meta().id).unwrap();
        let (applied, _) = exchange(&mut left, &mut right);
        let summary = applied.expect("left changed").summary;
        assert_eq!(summary.collections[0].deleted, 1);
        settle(&mut left, &mut right);
        assert!(left.doc.transactions.is_empty());
    }

    #[test]
    fn undo_puts_records_back_and_the_undo_travels() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        right
            .add_transaction(income(&cash, "20", "theirs"))
            .unwrap();
        right.doc.stores[0].name = "Renamed".into();
        right.persist().unwrap();
        let (applied, _) = exchange(&mut left, &mut right);
        let batch = applied.expect("left changed");
        assert_eq!(left.sync_history()[0].id, batch.id);
        let line = &left.sync_journal()[0];
        assert_eq!(line.batch_id.as_deref(), Some(batch.id.as_str()));
        assert_eq!(line.outcome, SyncJournalOutcome::Applied);

        let report = left.sync_undo(&batch.id).unwrap();
        assert_eq!(report.restored, 2);
        assert!(left.doc.transactions.is_empty());
        assert_eq!(left.doc.stores[0].name, "Cash");
        assert!(left.sync_history()[0].undone_at.is_some());
        assert!(left.sync_undo(&batch.id).is_err());

        settle(&mut left, &mut right);
        assert!(right.doc.transactions.is_empty());
        assert_eq!(right.doc.stores[0].name, "Cash");
    }

    #[test]
    fn undo_leaves_records_changed_since() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        right
            .add_transaction(income(&cash, "20", "theirs"))
            .unwrap();
        right.add_transaction(income(&cash, "30", "kept")).unwrap();
        let (applied, _) = exchange(&mut left, &mut right);
        let batch = applied.expect("left changed");
        let kept = left
            .doc
            .transactions
            .iter_mut()
            .find(|tx| tx.meta().comment == "kept")
            .unwrap();
        if let Transaction::Topup(topup) = kept {
            topup.meta.comment = "kept and edited".into();
        }
        left.persist().unwrap();
        let report = left.sync_undo(&batch.id).unwrap();
        assert_eq!(report.restored, 1);
        // The readable label of the record: its comment, date and amount.
        assert_eq!(report.skipped.len(), 1);
        assert!(
            report.skipped[0].starts_with("kept and edited"),
            "{:?}",
            report.skipped
        );
        assert_eq!(comments(&left), vec!["kept and edited"]);
    }

    #[test]
    fn an_automation_run_on_both_devices_books_once() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        let origin = TxOrigin::Automation {
            automation_id: "rent".into(),
            planned_at: "2026-11-01".into(),
        };
        for vault in [&mut left, &mut right] {
            let mut tx = income(&cash, "100", "rent");
            if let Transaction::Topup(topup) = &mut tx {
                topup.meta.origin = origin.clone();
            }
            vault.add_transaction(tx).unwrap();
        }
        settle(&mut left, &mut right);
        assert_eq!(left.doc.transactions.len(), 1);
        assert_eq!(right.doc.transactions, left.doc.transactions);
    }

    /// An automation's operation for one planned moment; its id.
    fn rent(vault: &mut UnlockedVault, comment: &str) -> String {
        let cash = cash_id(vault);
        let mut tx = income(&cash, "100", comment);
        if let Transaction::Topup(topup) = &mut tx {
            topup.meta.origin = TxOrigin::Automation {
                automation_id: "rent".into(),
                planned_at: "2026-11-01".into(),
            };
        }
        vault.add_transaction(tx).unwrap().meta().id.clone()
    }

    fn edit_comment(vault: &mut UnlockedVault, id: &str, text: &str) {
        for tx in &mut vault.doc.transactions {
            if let Transaction::Topup(topup) = tx
                && topup.meta.id == id
            {
                topup.meta.comment = text.into();
            }
        }
        vault.persist().unwrap();
    }

    fn pause() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    /// [`exchange`] with the right side still running the earlier dedupe.
    fn exchange_with_old_right(left: &mut UnlockedVault, right: &mut UnlockedVault) -> bool {
        let left_manifest = left.sync_manifest(None).unwrap();
        let right_manifest = right.sync_manifest(None).unwrap();
        let (left_wants, right_wants) = diff(&left_manifest, &right_manifest);
        let to_left = right.sync_records(&left_wants).unwrap();
        let to_right = left.sync_records(&right_wants).unwrap();
        let applied_left = left.sync_apply(&to_left, &peer("right")).unwrap();
        let applied_right = right.apply_batch(&to_right, &peer("left"), false).unwrap();
        applied_left.is_some() || applied_right.is_some()
    }

    #[test]
    fn dedupe_keeps_the_later_edit_of_either_copy() {
        // The copy made later is edited: its edit survives in the kept one.
        let (_dir, mut left, mut right) = two_devices();
        rent(&mut left, "rent");
        pause();
        let late = rent(&mut right, "rent");
        pause();
        edit_comment(&mut right, &late, "rent, corrected");
        settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["rent, corrected"]);
        assert_eq!(comments(&right), comments(&left));

        // The kept copy is edited after the other one: its edit stays.
        let (_dir, mut left, mut right) = two_devices();
        let early = rent(&mut left, "rent");
        pause();
        let late = rent(&mut right, "rent");
        pause();
        edit_comment(&mut right, &late, "from the later copy");
        pause();
        edit_comment(&mut left, &early, "from the kept copy");
        settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["from the kept copy"]);
        assert_eq!(comments(&right), comments(&left));
    }

    #[test]
    fn a_pair_with_the_earlier_dedupe_converges_to_one_record() {
        let (_dir, mut left, mut right) = two_devices();
        rent(&mut left, "rent");
        pause();
        let late = rent(&mut right, "rent");
        pause();
        edit_comment(&mut right, &late, "rent, corrected");
        let mut rounds = 0;
        while exchange_with_old_right(&mut left, &mut right) {
            rounds += 1;
            assert!(rounds <= 3, "the pair keeps deleting and recreating");
        }
        assert_eq!(comments(&left), vec!["rent, corrected"]);
        assert_eq!(comments(&right), comments(&left));
        assert_eq!(
            left.sync_manifest(None).unwrap(),
            right.sync_manifest(None).unwrap()
        );
    }

    #[test]
    fn dedupe_leaves_unstamped_imports_and_automations() {
        let (_dir, mut left, _right) = two_devices();
        let cash = cash_id(&left);
        for (index, origin) in [
            TxOrigin::Import {
                exec_id: "exec-1".into(),
            },
            TxOrigin::Automation {
                automation_id: "rent".into(),
                planned_at: "2026-11-01".into(),
            },
        ]
        .into_iter()
        .enumerate()
        {
            for copy in 0..2 {
                let mut tx = income(&cash, "100", &format!("{index}-{copy}"));
                if let Transaction::Topup(topup) = &mut tx {
                    topup.meta.origin = origin.clone();
                }
                left.doc.transactions.push(tx);
            }
        }
        left.doc.sync.stamps.clear();
        left.dedupe_generated("no-batch", true).unwrap();
        assert_eq!(
            left.doc.transactions.len(),
            4,
            "an unstamped one was deleted"
        );
    }

    #[test]
    fn undo_after_dedupe_restores_both_copies() {
        // Both copies are here when an unrelated batch arrives.
        let (_dir, mut left, mut right) = two_devices();
        rent(&mut left, "first");
        pause();
        let late = rent(&mut left, "second");
        pause();
        edit_comment(&mut left, &late, "second, edited");
        right.doc.stores[0].color = "#123456".into();
        right.persist().unwrap();
        let (applied_left, _) = exchange(&mut left, &mut right);
        let batch = applied_left.expect("left applied the store change");
        assert_eq!(comments(&left), vec!["second, edited"]);
        let report = left.sync_undo(&batch.id).unwrap();
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert_eq!(comments(&left), vec!["first", "second, edited"]);
        // Dedupe runs again with the next batch: the restored copy is a new
        // local change, later than the kept one, so it goes again. Undo
        // gives the copies back for a look; it does not outlaw the rule.
        right.doc.stores[0].name = "Renamed".into();
        right.persist().unwrap();
        settle(&mut left, &mut right);
        // The restored copy carries no field stamps (a restored record is
        // stamped new), so its edit is not carried a second time.
        assert_eq!(comments(&left), vec!["first"]);
        assert_eq!(comments(&right), comments(&left));

        // The other copy came in the batch itself: undo leaves the local one.
        let (_dir, mut left, mut right) = two_devices();
        rent(&mut right, "rent");
        pause();
        rent(&mut left, "rent");
        let (applied_left, _) = exchange(&mut left, &mut right);
        let batch = applied_left.expect("left took the earlier copy");
        assert_eq!(left.doc.transactions.len(), 1);
        left.sync_undo(&batch.id).unwrap();
        assert_eq!(left.doc.transactions.len(), 1, "undo left neither copy");
    }

    /// A workflow transaction of `run`, planned for `moment`.
    fn workflow_tx(vault: &mut UnlockedVault, run: &str, comment: &str) {
        let cash = cash_id(vault);
        let mut tx = income(&cash, "100", comment);
        if let Transaction::Topup(topup) = &mut tx {
            topup.meta.origin = TxOrigin::Workflow {
                workflow_id: "salary".into(),
                run_id: run.into(),
                planned_at: Some("2026-11-01".into()),
                estimated: false,
                run_key: None,
            };
        }
        vault.add_transaction(tx).unwrap();
    }

    /// Makes both devices apply something, so the dedupe runs on each.
    fn touch_and_settle(left: &mut UnlockedVault, right: &mut UnlockedVault) {
        left.doc.stores[0].name = "Left".into();
        left.persist().unwrap();
        right.doc.stores[0].color = "#123456".into();
        right.persist().unwrap();
        settle(left, right);
    }

    #[test]
    fn one_workflow_run_with_several_operations_for_one_moment_keeps_them_all() {
        let (_dir, mut left, mut right) = two_devices();
        workflow_tx(&mut left, "run-1", "salary");
        workflow_tx(&mut left, "run-1", "tax");
        touch_and_settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["salary", "tax"]);
        assert_eq!(comments(&right), comments(&left));
    }

    #[test]
    fn two_runs_on_one_device_for_one_moment_are_both_kept() {
        let (_dir, mut left, mut right) = two_devices();
        workflow_tx(&mut left, "run-1", "first");
        workflow_tx(&mut left, "run-2", "second");
        touch_and_settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["first", "second"]);
        assert_eq!(comments(&right), comments(&left));
    }

    #[test]
    fn the_same_moment_run_on_two_devices_keeps_the_earlier_run_whole() {
        let (_dir, mut left, mut right) = two_devices();
        workflow_tx(&mut left, "run-left", "left salary");
        workflow_tx(&mut left, "run-left", "left tax");
        std::thread::sleep(std::time::Duration::from_millis(5));
        workflow_tx(&mut right, "run-right", "right salary");
        workflow_tx(&mut right, "run-right", "right tax");
        settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["left salary", "left tax"]);
        assert_eq!(comments(&right), comments(&left));
    }

    #[test]
    fn a_deletion_that_arrives_late_survives_the_compaction_of_the_same_apply() {
        let (_dir, mut left, _right) = two_devices();
        let old = crate::sync::stamp::now_ms() - 200 * 24 * 3600 * 1000;
        // The record is still here (stamped before the deletion), so the
        // deletion is news; one for an unknown key is dropped (see
        // `tombstone_tests`).
        let contact = left
            .create_contact(crate::service::contacts::ContactDraft {
                name: "Gone".into(),
                kind: crate::model::contact::ContactKind::Person,
                note: String::new(),
                accounts: Vec::new(),
                group: String::new(),
            })
            .unwrap();
        let key = format!("contacts:{}", contact.id);
        left.doc.sync.stamps.insert(
            key.clone(),
            crate::sync::RecordStamps::created(Stamp::new(old - 1000, 0, "aaaaaaaa")),
        );
        left.persist().unwrap();
        let incoming: BTreeMap<String, RecordState> = [(
            key.clone(),
            RecordState {
                value: None,
                stamps: None,
                tombstone: Some(Stamp::new(old, 0, "bbbbbbbb")),
            },
        )]
        .into();
        left.sync_apply(&incoming, &peer("right"))
            .unwrap()
            .expect("taken");
        assert!(left.doc.sync.tombstones.contains_key(&key));
        assert!(left.doc.sync.tombstones_seen.contains_key(&key));
        // The bookkeeping is not part of what is described to others.
        assert!(left.sync_manifest(None).unwrap().contains_key(&key));
    }

    #[test]
    fn a_compacted_deletion_takes_its_seen_time_with_it() {
        let (_dir, mut left, _right) = two_devices();
        let now = crate::sync::stamp::now_ms();
        let old = now - 400 * 24 * 3600 * 1000;
        let key = "contacts:gone".to_string();
        left.doc
            .sync
            .tombstones
            .insert(key.clone(), Stamp::new(old, 0, "bbbbbbbb"));
        left.doc.sync.tombstones_seen.insert(key.clone(), old);
        left.compact_tombstones();
        assert!(!left.doc.sync.tombstones.contains_key(&key));
        assert!(!left.doc.sync.tombstones_seen.contains_key(&key));
    }

    #[test]
    fn device_settings_never_travel() {
        let (_dir, mut left, mut right) = two_devices();
        left.doc.settings.minimize_to_tray = false;
        left.doc.settings.savings_rate = crate::Amount::parse("15").unwrap();
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert!(right.doc.settings.minimize_to_tray);
        assert_eq!(
            right.doc.settings.savings_rate,
            left.doc.settings.savings_rate
        );
    }

    #[test]
    fn a_record_under_a_foreign_key_is_refused() {
        let (_dir, mut left, mut right) = two_devices();
        let mut forged = right
            .sync_records(&[format!("stores:{}", cash_id(&right))])
            .unwrap();
        let state = forged.values().next().unwrap().clone();
        forged.clear();
        forged.insert("stores:someone-else".into(), state);
        assert!(matches!(
            left.sync_apply(&forged, &peer("right")),
            Err(Error::InvalidInput(_))
        ));
        forged.clear();
        forged.insert("holdings:x".into(), RecordState::default());
        assert!(left.sync_preview(&forged).is_err());
    }

    #[test]
    fn a_record_that_does_not_fit_the_schema_changes_nothing() {
        let (_dir, mut left, _right) = two_devices();
        let before = left.doc.clone();
        let broken: BTreeMap<String, RecordState> = [(
            "stores:bad".to_string(),
            RecordState {
                value: Some(serde_json::json!({"id": "bad", "kind": 42})),
                stamps: Some(Default::default()),
                tombstone: None,
            },
        )]
        .into();
        assert!(left.sync_apply(&broken, &peer("right")).is_err());
        assert_eq!(left.doc, before);
    }

    #[test]
    fn json_editor_edits_are_stamped_and_keep_the_bookkeeping() {
        let (_dir, mut left, mut right) = two_devices();
        left.sync_trust_peer("dev-b", "Phone", "aa").unwrap();
        let mut edited = left.doc.clone();
        edited.stores[0].name = "Edited".into();
        edited.sync = Default::default();
        left.replace_doc(edited).unwrap();
        assert_eq!(left.sync_state().peers.len(), 1);
        settle(&mut left, &mut right);
        assert_eq!(right.doc.stores[0].name, "Edited");
    }

    #[test]
    fn forgetting_a_device_reaches_the_others() {
        let (_dir, mut left, mut right) = two_devices();
        left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
        right
            .sync_merge_peers(&left.sync_state().peers.clone())
            .unwrap();
        assert!(right.sync_state().trusted("cc").is_some());
        left.sync_forget_peer("dev-c").unwrap();
        right
            .sync_merge_peers(&left.sync_state().peers.clone())
            .unwrap();
        assert!(right.sync_state().trusted("cc").is_none());
        assert!(right.sync_state().is_revoked("cc"));
    }

    #[test]
    fn trusting_a_forgotten_device_again_clears_the_revocation() {
        let (_dir, mut left, _right) = two_devices();
        left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
        left.sync_forget_peer("dev-c").unwrap();
        assert!(left.sync_state().is_revoked("cc"));
        left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
        assert!(!left.sync_state().is_revoked("cc"));
        assert!(left.sync_state().trusted("cc").is_some());
    }

    #[test]
    fn an_older_revoked_entry_under_another_device_id_does_not_revoke_a_new_trust() {
        let (_dir, mut left, _right) = two_devices();
        left.sync_trust_peer("dev-old", "Tablet", "cc").unwrap();
        left.sync_forget_peer("dev-old").unwrap();
        left.sync_trust_peer("dev-new", "Tablet", "cc").unwrap();
        assert!(!left.sync_state().is_revoked("cc"));
        assert_eq!(
            left.sync_state().trusted("cc").unwrap().device_id,
            "dev-new"
        );
    }

    #[test]
    fn a_merged_older_revocation_does_not_beat_a_newer_trust() {
        let (_dir, mut left, mut right) = two_devices();
        left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
        left.sync_forget_peer("dev-c").unwrap();
        let revoked = left.sync_state().peers.clone();
        right.sync_merge_peers(&revoked).unwrap();
        right.sync_trust_peer("dev-c2", "Tablet", "cc").unwrap();
        right.sync_merge_peers(&revoked).unwrap();
        assert!(right.sync_state().trusted("cc").is_some());
        assert!(!right.sync_state().is_revoked("cc"));
    }

    #[test]
    fn pairing_after_a_merged_future_revocation_still_sorts_newer() {
        let (_dir, mut left, _right) = two_devices();
        let ahead = crate::sync::stamp::now_ms() + 3_600_000;
        let revoked = TrustedPeer {
            device_id: "dev-c".into(),
            public_key: "cc".into(),
            revoked: Some(Stamp::new(ahead, 0, "remote")),
            stamp: Stamp::new(ahead, 0, "remote"),
            ..TrustedPeer::default()
        };
        assert!(left.sync_merge_peers(&[revoked]).unwrap());
        assert!(left.sync_state().is_revoked("cc"));
        left.sync_trust_peer("dev-c", "Tablet", "cc").unwrap();
        assert!(left.sync_state().trusted("cc").is_some());
    }

    #[test]
    fn forgetting_the_newest_entry_still_revokes_the_key() {
        let (_dir, mut left, _right) = two_devices();
        left.sync_trust_peer("dev-old", "Tablet", "cc").unwrap();
        left.sync_forget_peer("dev-old").unwrap();
        left.sync_trust_peer("dev-new", "Tablet", "cc").unwrap();
        left.sync_forget_peer("dev-new").unwrap();
        assert!(left.sync_state().is_revoked("cc"));
        assert!(left.sync_state().trusted("cc").is_none());
    }

    #[test]
    fn the_backup_restores_the_vault_as_before_the_sync() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        right
            .add_transaction(income(&cash, "20", "theirs"))
            .unwrap();
        left.sync_backup().unwrap();
        exchange(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["theirs"]);
        left.sync_restore_backup().unwrap();
        assert!(left.doc.transactions.is_empty());
        settle(&mut left, &mut right);
        assert!(right.doc.transactions.is_empty());
    }

    #[test]
    fn a_vault_from_before_sync_opens_and_starts_stamping() {
        let (dir, mut vault) = empty_vault();
        vault.doc.sync = Default::default();
        vault.persist().unwrap();
        let path = vault.path.clone();
        drop(vault);
        let mut reopened = UnlockedVault::open(path, "pw").unwrap();
        assert!(reopened.sync_state().stamps.is_empty());
        reopened.doc.stores[0].name = "X".into();
        reopened.persist().unwrap();
        assert_eq!(reopened.sync_state().stamps.len(), 1);
        drop(dir);
    }

    #[test]
    fn handing_the_vault_over_needs_its_password_and_leaves_a_trace() {
        let (_dir, mut vault) = empty_vault();
        assert!(vault.check_password("pw").is_ok());
        assert!(matches!(vault.check_password("pw "), Err(Error::Secret)));
        assert!(matches!(vault.check_password(""), Err(Error::Secret)));

        let notice = vault
            .sync_record_transfer("phone", "Сочный Апельсин", "ab", true)
            .unwrap();
        assert_eq!(
            notice.category,
            crate::model::notify::NoticeCategory::Security
        );
        assert!(notice.body.contains("Сочный Апельсин"));
        // Every linked device gets the notice (it is in the synced log)...
        assert!(
            vault
                .doc
                .notification_log
                .iter()
                .any(|record| record.key == notice.key)
        );
        // ...the new device may sync...
        assert!(vault.sync_state().trusted("ab").is_some());
        // ...and the history says where the vault went.
        let history = vault.sync_history();
        assert_eq!(
            history[0].transferred_to.as_deref(),
            Some("Сочный Апельсин")
        );
    }
    #[test]
    fn a_pause_survives_a_reopen_and_is_never_a_peer_field() {
        let (dir, mut vault) = empty_vault();
        assert!(!vault.sync_is_paused("dev-a"));
        vault.sync_pause_peer("dev-a").unwrap();
        // Pausing twice changes nothing.
        vault.sync_pause_peer("dev-a").unwrap();
        assert!(vault.sync_is_paused("dev-a"));
        assert!(!vault.sync_is_paused("dev-b"));
        let reopened = UnlockedVault::open(vault.path.clone(), "pw").unwrap();
        assert!(reopened.sync_is_paused("dev-a"));
        // The list that goes to other devices does not carry it.
        assert!(reopened.sync_state().peers.is_empty());
        let mut reopened = reopened;
        reopened.sync_resume_peer("dev-a").unwrap();
        assert!(!reopened.sync_is_paused("dev-a"));
        let again = UnlockedVault::open(reopened.path.clone(), "pw").unwrap();
        assert!(!again.sync_is_paused("dev-a"));
        drop(dir);
    }

    #[test]
    fn forgetting_a_device_lifts_its_pause() {
        let (_dir, mut vault) = empty_vault();
        vault.sync_trust_peer("dev-a", "A", "key-a").unwrap();
        vault.sync_pause_peer("dev-a").unwrap();
        vault.sync_forget_peer("dev-a").unwrap();
        assert!(!vault.sync_is_paused("dev-a"));
    }

    #[test]
    fn merging_a_peer_list_leaves_the_pause_alone() {
        let (_dir, mut vault) = empty_vault();
        vault.sync_trust_peer("dev-a", "A", "key-a").unwrap();
        vault.sync_pause_peer("dev-a").unwrap();
        let remote = vault.sync_state().peers.clone();
        vault.sync_merge_peers(&remote).unwrap();
        assert!(vault.sync_is_paused("dev-a"));
    }

    #[test]
    fn cut_by_is_saved_once_survives_a_reopen_and_is_never_a_peer_field() {
        let (dir, mut vault) = empty_vault();
        assert_eq!(vault.sync_cut_reason("dev-a"), None);
        assert!(vault.sync_cut_by("dev-a", SyncCutReason::Paused).unwrap());
        // The same news again changes nothing; another reason does.
        assert!(!vault.sync_cut_by("dev-a", SyncCutReason::Paused).unwrap());
        let reopened = UnlockedVault::open(vault.path.clone(), "pw").unwrap();
        assert_eq!(
            reopened.sync_cut_reason("dev-a"),
            Some(SyncCutReason::Paused)
        );
        assert!(reopened.sync_state().peers.is_empty());
        let mut reopened = reopened;
        assert!(
            reopened
                .sync_cut_by("dev-a", SyncCutReason::Forgotten)
                .unwrap()
        );
        // A merged list leaves it alone.
        let remote = reopened.sync_state().peers.clone();
        reopened.sync_merge_peers(&remote).unwrap();
        assert_eq!(
            reopened.sync_cut_reason("dev-a"),
            Some(SyncCutReason::Forgotten)
        );
        assert!(reopened.sync_clear_cut("dev-a").unwrap());
        assert!(!reopened.sync_clear_cut("dev-a").unwrap());
        drop(dir);
    }

    #[test]
    fn pairing_or_forgetting_a_device_clears_what_it_told_us() {
        let (_dir, mut vault) = empty_vault();
        vault
            .sync_cut_by("dev-a", SyncCutReason::Forgotten)
            .unwrap();
        vault.sync_trust_peer("dev-a", "A", "key-a").unwrap();
        assert_eq!(vault.sync_cut_reason("dev-a"), None);
        vault.sync_cut_by("dev-a", SyncCutReason::Paused).unwrap();
        vault.sync_forget_peer("dev-a").unwrap();
        assert_eq!(vault.sync_cut_reason("dev-a"), None);
    }
}
