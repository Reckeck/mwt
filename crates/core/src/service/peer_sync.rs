//! The vault side of device-to-device sync: stamping local changes on every
//! save, describing the records to another device, applying and undoing what
//! comes back (docs/sync.md). The pure parts live in [`crate::sync`].

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::UnlockedVault;
use crate::error::{Error, Result};
use crate::model::transaction::TxOrigin;
use crate::model::vault::{VaultDoc, now_iso};
use crate::storage::VaultFile;
use crate::sync::manifest::{Manifest, digest};
use crate::sync::registry::Rule;
use crate::sync::state::{BATCH_DAYS, MAX_BATCHES, TOMBSTONE_DAYS};
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
        if let Some(latest) = self.doc.sync.latest_stamp().cloned() {
            self.clock.observe(&latest);
        }
        Ok(())
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
            ..
        } = self;
        match tracker {
            Some(tracker) => Ok(tracker.stamp(&value, &mut doc.sync, clock)),
            None => {
                *tracker = Some(Tracker::capture(&value));
                Ok(Vec::new())
            }
        }
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

    /// One digest per record and deletion. `union_since_ms` leaves out cached
    /// prices and rates older than that (a first sync sends 90 days).
    pub fn sync_manifest(&mut self, union_since_ms: Option<u64>) -> Result<Manifest> {
        self.track_changes()?;
        let value = self.doc_value()?;
        let records = view::records(&value);
        let mut manifest = Manifest::new();
        for key in records.keys() {
            if let (Some(since), Some((coll, _))) = (union_since_ms, split_key(key))
                && let Some(field) = coll.age
                && let Some(Value::String(at)) = records[key].get(field)
                && u64::try_from(crate::model::vault::iso_to_millis(at)).unwrap_or(0) < since
            {
                continue;
            }
            manifest.insert(key.clone(), digest(&self.state_of(&records, key)));
        }
        for key in self.doc.sync.tombstones.keys() {
            if !records.contains_key(key) {
                manifest.insert(key.clone(), digest(&self.state_of(&records, key)));
            }
        }
        Ok(manifest)
    }

    /// The full state of each asked record (absent keys come back empty).
    pub fn sync_records(&mut self, keys: &[String]) -> Result<BTreeMap<String, RecordState>> {
        self.track_changes()?;
        let value = self.doc_value()?;
        let records = view::records(&value);
        Ok(keys
            .iter()
            .filter(|key| split_key(key).is_some())
            .map(|key| (key.clone(), self.state_of(&records, key)))
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

    /// Merge records from another device and save. `None` when nothing
    /// changed here; otherwise the batch, kept for undo.
    pub fn sync_apply(
        &mut self,
        incoming: &BTreeMap<String, RecordState>,
        peer: &PeerRef,
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
        let plan = crate::sync::plan::plan(incoming, |key| self.state_of(&records, key));
        if plan.changes.is_empty() {
            return Ok(None);
        }
        for (key, change) in &plan.changes {
            view::write(&mut value, key, change.after.value.as_ref());
        }
        view::trim(&mut value);
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
                Some(stamp) => doc.sync.tombstones.insert(key.clone(), stamp.clone()),
                None => doc.sync.tombstones.remove(key),
            };
        }
        // Log entries the cap cut off keep no stamps.
        let kept = view::records(&value);
        doc.sync.stamps.retain(|key, _| kept.contains_key(key));
        self.doc = doc;
        // The typed document may serialize a little differently from the
        // merged JSON (defaults, number forms): the tracker starts from what
        // a save will actually see, or the next save would restamp the batch.
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
        self.doc.sync.batches.push(batch);
        self.prune_batches();
        self.compact_tombstones();
        // Both devices may have run the same automation or import before
        // they met; the copy made later goes (as an ordinary deletion).
        self.dedupe_generated();
        self.recompute_holdings();
        self.persist()?;
        Ok(Some(applied))
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
        for entry in &entries {
            let current = self.state_of(&records, &entry.key);
            if current.same_marks(&entry.after) {
                view::write(&mut value, &entry.key, entry.before.value.as_ref());
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
        let mut doc: VaultDoc = serde_json::from_value(value)
            .map_err(|err| Error::InvalidInput(format!("sync undo: {err}")))?;
        doc.sync = std::mem::take(&mut self.doc.sync);
        doc.sync.batches[index].undone_at = Some(now_iso());
        self.doc = doc;
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

    /// Forget deletions old enough that every trusted device has synced since.
    fn compact_tombstones(&mut self) {
        let cutoff = crate::sync::stamp::now_ms().saturating_sub(day_ms(TOMBSTONE_DAYS));
        let peers: Vec<u64> = self
            .doc
            .sync
            .peers
            .iter()
            .filter(|peer| peer.revoked.is_none())
            .map(|peer| {
                peer.last_synced_at
                    .as_deref()
                    .map(crate::model::vault::iso_to_millis)
                    .and_then(|ms| u64::try_from(ms).ok())
                    .unwrap_or(0)
            })
            .collect();
        self.doc.sync.tombstones.retain(|_, stamp| {
            stamp.wall >= cutoff || peers.iter().any(|synced| *synced <= stamp.wall)
        });
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
    /// (once per device): the one created first, then the smaller id.
    fn dedupe_generated(&mut self) {
        let mut best: HashMap<String, (Stamp, String)> = HashMap::new();
        for tx in &self.doc.transactions {
            let meta = tx.meta();
            let natural = match &meta.origin {
                TxOrigin::Import { exec_id } => format!("import:{exec_id}"),
                TxOrigin::Automation {
                    automation_id,
                    planned_at,
                } => format!("auto:{automation_id}:{planned_at}"),
                _ => continue,
            };
            let rank = (
                self.created_of(&format!("transactions:{}", meta.id)),
                meta.id.clone(),
            );
            best.entry(natural)
                .and_modify(|kept| {
                    if rank < *kept {
                        *kept = rank.clone();
                    }
                })
                .or_insert(rank);
        }
        let keep: std::collections::HashSet<String> =
            best.into_values().map(|(_, id)| id).collect();
        self.doc.transactions.retain(|tx| {
            let meta = tx.meta();
            matches!(meta.origin, TxOrigin::Manual | TxOrigin::Workflow { .. })
                || keep.contains(&meta.id)
        });

        let mut seen = std::collections::HashSet::new();
        let mut runs = std::mem::take(&mut self.doc.automation_runs);
        runs.sort_by(|left, right| left.id.cmp(&right.id));
        runs.retain(|run| seen.insert((run.automation_id.clone(), run.planned_at.clone())));
        runs.sort_by(|left, right| left.processed_at.cmp(&right.processed_at));
        self.doc.automation_runs = runs;

        let mut first: HashMap<String, (Stamp, String)> = HashMap::new();
        for reminder in &self.doc.reminders {
            if reminder.source_key.is_empty() {
                continue;
            }
            let rank = (
                self.created_of(&format!("reminders:{}", reminder.id)),
                reminder.id.clone(),
            );
            first
                .entry(reminder.source_key.clone())
                .and_modify(|kept| {
                    if rank < *kept {
                        *kept = rank.clone();
                    }
                })
                .or_insert(rank);
        }
        let keep: std::collections::HashSet<String> =
            first.into_values().map(|(_, id)| id).collect();
        self.doc
            .reminders
            .retain(|reminder| reminder.source_key.is_empty() || keep.contains(&reminder.id));
    }

    // -- trusted devices ------------------------------------------------------

    /// Trust a device after both sides confirmed the pairing code.
    pub fn sync_trust_peer(&mut self, device_id: &str, name: &str, public_key: &str) -> Result<()> {
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
        self.persist()
    }

    /// «Забыть устройство»: its key is refused here and, once this reaches
    /// them, on every other device.
    pub fn sync_forget_peer(&mut self, device_id: &str) -> Result<()> {
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
        self.persist()
    }

    /// Take in another device's list of trusted devices.
    pub fn sync_merge_peers(&mut self, remote: &[TrustedPeer]) -> Result<bool> {
        for peer in remote {
            self.clock.observe(&peer.stamp);
        }
        let changed = self.doc.sync.merge_peers(remote);
        if changed {
            self.persist()?;
        }
        Ok(changed)
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
    pub fn sync_backup(&self) -> Result<PathBuf> {
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
        std::fs::copy(&self.path, &target).map_err(|err| Error::Io(err.to_string()))?;
        Ok(target)
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
mod tests {
    use super::super::test_support::{empty_vault, income};
    use super::*;
    use crate::model::transaction::Transaction;
    use crate::sync::manifest::diff;

    /// The same vault on two devices: a copy of one file, opened twice.
    fn two_devices() -> (tempfile::TempDir, UnlockedVault, UnlockedVault) {
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
    fn exchange(
        left: &mut UnlockedVault,
        right: &mut UnlockedVault,
    ) -> (Option<AppliedBatch>, Option<AppliedBatch>) {
        let left_manifest = left.sync_manifest(None).unwrap();
        let right_manifest = right.sync_manifest(None).unwrap();
        let (left_wants, right_wants) = diff(&left_manifest, &right_manifest);
        let to_left = right.sync_records(&left_wants).unwrap();
        let to_right = left.sync_records(&right_wants).unwrap();
        let applied_left = left.sync_apply(&to_left, &peer("right")).unwrap();
        let applied_right = right.sync_apply(&to_right, &peer("left")).unwrap();
        (applied_left, applied_right)
    }

    /// Exchange until nothing moves; both sides must then agree.
    fn settle(left: &mut UnlockedVault, right: &mut UnlockedVault) {
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
        assert_eq!(report.skipped, vec!["kept and edited".to_string()]);
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
}
