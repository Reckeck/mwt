//! What a vault keeps about sync: change stamps, deletions, trusted devices
//! and the batches it took in (for undo). Stored in [`crate::VaultDoc::sync`].

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::stamp::Stamp;

/// Stamps of one record: when it was created and, for each field changed
/// since, when that field last changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordStamps {
    #[serde(rename = "c")]
    pub created: Stamp,
    #[serde(rename = "f", default)]
    pub fields: BTreeMap<String, Stamp>,
}

impl RecordStamps {
    pub fn created(created: Stamp) -> Self {
        Self {
            created,
            fields: BTreeMap::new(),
        }
    }

    /// When `field` last changed.
    pub fn field(&self, field: &str) -> &Stamp {
        self.fields.get(field).unwrap_or(&self.created)
    }

    /// The latest change to the record.
    pub fn latest(&self) -> &Stamp {
        self.fields
            .values()
            .chain(std::iter::once(&self.created))
            .max()
            .unwrap_or(&self.created)
    }
}

/// One record as sync sees it: its value (none once deleted), its stamps and
/// the stamp of its deletion.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RecordState {
    /// Kept as JSON text when stored or sent: every vault format (bincode
    /// included) can carry a string, not all can carry free-form JSON.
    #[serde(default, with = "json_text")]
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub stamps: Option<RecordStamps>,
    #[serde(default)]
    pub tombstone: Option<Stamp>,
}

impl RecordState {
    /// The same state without the value: what undo compares against.
    pub fn marks(&self) -> RecordState {
        RecordState {
            value: None,
            stamps: self.stamps.clone(),
            tombstone: self.tombstone.clone(),
        }
    }

    /// Whether `other` carries the same stamps (the value is not compared:
    /// a live record always has stamps, a deleted one never does).
    pub fn same_marks(&self, other: &RecordState) -> bool {
        self.stamps == other.stamps && self.tombstone == other.tombstone
    }

    /// The latest change this state knows of, deletion included.
    pub fn latest(&self) -> Option<&Stamp> {
        let changed = self.stamps.as_ref().map(RecordStamps::latest);
        match (changed, self.tombstone.as_ref()) {
            (Some(changed), Some(deleted)) => Some(changed.max(deleted)),
            (changed, deleted) => changed.or(deleted),
        }
    }
}

mod json_text {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::Value;

    pub fn serialize<S: Serializer>(value: &Option<Value>, out: S) -> Result<S::Ok, S::Error> {
        value.as_ref().map(Value::to_string).serialize(out)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(input: D) -> Result<Option<Value>, D::Error> {
        Option::<String>::deserialize(input)?
            .map(|text| serde_json::from_str(&text).map_err(serde::de::Error::custom))
            .transpose()
    }
}

/// A device allowed to sync this vault.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrustedPeer {
    pub device_id: String,
    pub name: String,
    /// Its static Noise key, hex.
    pub public_key: String,
    /// RFC3339.
    pub paired_at: String,
    /// RFC3339 of the last batch exchanged with it.
    #[serde(default)]
    pub last_synced_at: Option<String>,
    /// Set by "Forget device" on any device; a revoked key is refused until it is paired again (a newer entry).
    #[serde(default)]
    pub revoked: Option<Stamp>,
    /// When this entry last changed; the later entry wins between devices.
    #[serde(default)]
    pub stamp: Stamp,
}

/// One record a batch changed, for undo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BatchEntry {
    pub key: String,
    /// The record before the batch.
    pub before: RecordState,
    /// The stamps the batch left (no value): undo touches the record only
    /// while they are still the record's stamps.
    pub after: RecordState,
}

/// What one incoming batch changed here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncBatch {
    pub id: String,
    pub peer_device: String,
    pub peer_name: String,
    /// RFC3339.
    pub applied_at: String,
    pub summary: super::plan::SyncSummary,
    pub entries: Vec<BatchEntry>,
    /// RFC3339 of the undo, once undone.
    #[serde(default)]
    pub undone_at: Option<String>,
}

/// Batches kept for undo: at most this many…
pub const MAX_BATCHES: usize = 20;
/// …and none older than this many days.
pub const BATCH_DAYS: i64 = 7;
/// Deletions older than this are forgotten once every trusted device has synced since.
pub const TOMBSTONE_DAYS: u64 = 180;

/// Sync bookkeeping of a vault.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncState {
    /// `collection:key` → stamps. A record without an entry was last changed
    /// before sync existed (stamp zero).
    #[serde(default)]
    pub stamps: BTreeMap<String, RecordStamps>,
    /// `collection:key` → when it was deleted.
    #[serde(default)]
    pub tombstones: BTreeMap<String, Stamp>,
    /// `collection:key` → local Unix ms when this device took the deletion in.
    /// Bookkeeping like the rest: a deletion that arrives late is kept for the
    /// full retention from then, not from when it was made. Never synced.
    #[serde(default)]
    pub tombstones_seen: BTreeMap<String, u64>,
    #[serde(default)]
    pub peers: Vec<TrustedPeer>,
    /// Newest last.
    #[serde(default)]
    pub batches: Vec<SyncBatch>,
    /// Times this vault was handed to a new device from here, newest last.
    #[serde(default)]
    pub transfers: Vec<TransferRecord>,
    /// The journal of taken batches, newest last (at most `MAX_JOURNAL`).
    #[serde(default)]
    pub journal: Vec<super::journal::SyncJournalEntry>,
    /// Device ids whose exchange the user paused ("Decline" on a list of
    /// changes). Local bookkeeping: never synced, never part of `peers`.
    #[serde(default)]
    pub paused: BTreeSet<String>,
    /// Devices that cut this one off on purpose (they paused it or forgot it),
    /// as they told us. Local bookkeeping: never synced, never part of `peers`.
    #[serde(default)]
    pub cut_by: BTreeMap<String, CutBy>,
}

/// Why and when another device refused this one on purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutBy {
    pub reason: crate::ipc_contract::SyncCutReason,
    /// RFC3339 of the first time it told us.
    pub at: String,
}

/// The vault was handed to a new device from this one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferRecord {
    pub id: String,
    pub device_id: String,
    pub device_name: String,
    /// RFC3339.
    pub at: String,
}

/// Transfers kept in the history.
pub const MAX_TRANSFERS: usize = 50;

impl SyncState {
    /// Adopt the record metadata (stamps, deletions) and the trusted devices of
    /// `remote`, keeping every other field of `self` as it is. The record
    /// metadata describes the data that arrives with `remote`; everything else
    /// (undo batches, journal, transfers, `tombstones_seen`, `paused`, `cut_by`)
    /// is this device's own bookkeeping. The one place that knows which fields
    /// are device-local: a pull of an external copy keeps the current state
    /// here, a received file starts from [`SyncState::default`].
    pub fn take_records_from(&mut self, remote: SyncState, now: u64) {
        self.tombstones_seen
            .retain(|key, _| remote.tombstones.contains_key(key));
        for key in remote.tombstones.keys() {
            self.tombstones_seen.entry(key.clone()).or_insert(now);
        }
        self.stamps = remote.stamps;
        self.tombstones = remote.tombstones;
        self.merge_peers(&remote.peers, now);
    }

    /// The latest stamp anywhere, to start the clock after it.
    pub fn latest_stamp(&self) -> Option<&Stamp> {
        self.stamps
            .values()
            .map(RecordStamps::latest)
            .chain(self.tombstones.values())
            .chain(self.peers.iter().map(|peer| &peer.stamp))
            .max()
    }

    /// The newest entry for a key: it decides whether the key is trusted.
    fn latest_entry(&self, public_key: &str) -> Option<&TrustedPeer> {
        self.peers
            .iter()
            .filter(|peer| peer.public_key == public_key)
            .max_by(|left, right| left.stamp.cmp(&right.stamp))
    }

    /// A trusted peer by its key: the newest entry for the key, if not revoked.
    pub fn trusted(&self, public_key: &str) -> Option<&TrustedPeer> {
        self.latest_entry(public_key)
            .filter(|peer| peer.revoked.is_none())
    }

    /// Whether this key was trusted once and then forgotten: its newest entry
    /// is revoked. A later pairing makes a newer entry and lifts it.
    pub fn is_revoked(&self, public_key: &str) -> bool {
        self.latest_entry(public_key)
            .is_some_and(|peer| peer.revoked.is_some())
    }

    /// Take in another device's list: per device, the later entry wins, with
    /// two limits. An entry stamped further ahead of `now` than a clock can be
    /// is skipped. Revoke and trust are decided by stamp alone: a re-pairing
    /// (a newer, live entry) lifts an older revocation, an older entry never
    /// lifts a newer one. A live entry cannot swap the key of a live one.
    pub fn merge_peers(&mut self, remote: &[TrustedPeer], now: u64) -> bool {
        let limit = now.saturating_add(super::session::SKEW_REFUSE_MS);
        let mut changed = false;
        for theirs in remote {
            if theirs.stamp.wall > limit {
                continue;
            }
            let position = self
                .peers
                .iter()
                .position(|mine| mine.device_id == theirs.device_id);
            match position {
                Some(index) => {
                    let mine = &self.peers[index];
                    let accept = theirs.stamp > mine.stamp
                        && (theirs.revoked.is_some()
                            || mine.revoked.is_some()
                            || mine.public_key == theirs.public_key);
                    if accept {
                        self.peers[index] = theirs.clone();
                        changed = true;
                    }
                }
                None => {
                    if theirs.revoked.is_none() && self.is_revoked(&theirs.public_key) {
                        continue;
                    }
                    self.peers.push(theirs.clone());
                    changed = true;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000_000;

    fn entry(device_id: &str, key: &str, wall: u64, revoked: bool) -> TrustedPeer {
        let stamp = Stamp::new(wall, 0, "remote");
        TrustedPeer {
            device_id: device_id.into(),
            name: device_id.into(),
            public_key: key.into(),
            revoked: revoked.then(|| stamp.clone()),
            stamp,
            ..TrustedPeer::default()
        }
    }

    fn state_with(peers: Vec<TrustedPeer>) -> SyncState {
        SyncState {
            peers,
            ..SyncState::default()
        }
    }

    #[test]
    fn a_merged_list_never_lifts_a_revocation_with_an_older_entry() {
        let mut state = state_with(vec![entry("dev", "kk", 20, true)]);
        // An older, not revoked entry for the same device is refused.
        assert!(!state.merge_peers(&[entry("dev", "kk", 10, false)], NOW));
        assert!(state.is_revoked("kk"));
        // A newer re-pairing lifts it.
        assert!(state.merge_peers(&[entry("dev", "kk", 30, false)], NOW));
        assert!(!state.is_revoked("kk"));
        // A key change on a live entry is refused.
        let mut live = state_with(vec![entry("dev", "kk", 10, false)]);
        assert!(!live.merge_peers(&[entry("dev", "other", 20, false)], NOW));
        assert_eq!(live.peers[0].public_key, "kk");
    }

    #[test]
    fn a_newer_revocation_is_taken_and_a_newer_live_entry_for_a_live_one_too() {
        let mut state = state_with(vec![entry("dev", "kk", 10, false)]);
        assert!(state.merge_peers(&[entry("dev", "kk", 20, false)], NOW));
        assert_eq!(state.peers[0].stamp.wall, 20);
        assert!(state.merge_peers(&[entry("dev", "kk", 30, true)], NOW));
        assert!(state.is_revoked("kk"));
    }

    #[test]
    fn an_unknown_device_for_a_revoked_key_is_not_added_unless_revoked_itself() {
        let mut state = state_with(vec![entry("dev", "kk", 10, true)]);
        assert!(!state.merge_peers(&[entry("clone", "kk", 20, false)], NOW));
        assert_eq!(state.peers.len(), 1);
        assert!(state.merge_peers(&[entry("clone", "kk", 20, true)], NOW));
        assert_eq!(state.peers.len(), 2);
        assert!(state.merge_peers(&[entry("fresh", "new", 20, false)], NOW));
        assert!(state.trusted("new").is_some());
    }

    #[test]
    fn an_entry_stamped_far_in_the_future_is_skipped() {
        let mut state = state_with(vec![entry("dev", "kk", 10, false)]);
        let ahead = NOW + super::super::session::SKEW_REFUSE_MS + 1;
        assert!(!state.merge_peers(&[entry("dev", "kk", ahead, true)], NOW));
        assert!(!state.merge_peers(&[entry("other", "oo", ahead, false)], NOW));
        assert_eq!(state.peers.len(), 1);
        assert!(state.trusted("kk").is_some());
    }

    #[test]
    fn state_saved_without_tombstones_seen_still_loads() {
        let state: SyncState = serde_json::from_str(r#"{"stamps":{},"tombstones":{}}"#).unwrap();
        assert!(state.tombstones_seen.is_empty());
        let bare: SyncState = serde_json::from_str("{}").unwrap();
        assert_eq!(bare, SyncState::default());
    }

    #[test]
    fn the_latest_stamp_includes_the_peers() {
        let state = state_with(vec![entry("dev", "kk", 77, false)]);
        assert_eq!(state.latest_stamp().map(|stamp| stamp.wall), Some(77));
    }
    #[test]
    fn the_pause_round_trips_in_json_and_messagepack() {
        let mut state = SyncState::default();
        state.paused.insert("dev-a".into());
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(serde_json::from_str::<SyncState>(&json).unwrap(), state);
        let pack = rmp_serde::to_vec_named(&state).unwrap();
        assert_eq!(rmp_serde::from_slice::<SyncState>(&pack).unwrap(), state);
        // A vault saved before the field existed opens with nobody paused.
        let old: SyncState = serde_json::from_str(r#"{"peers":[]}"#).unwrap();
        assert!(old.paused.is_empty());
    }

    #[test]
    fn cut_by_round_trips_in_json_and_messagepack_and_old_vaults_open() {
        let mut state = SyncState::default();
        state.cut_by.insert(
            "dev-a".into(),
            CutBy {
                reason: crate::ipc_contract::SyncCutReason::Paused,
                at: "2026-11-02T10:00:00Z".into(),
            },
        );
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(serde_json::from_str::<SyncState>(&json).unwrap(), state);
        let pack = rmp_serde::to_vec_named(&state).unwrap();
        assert_eq!(rmp_serde::from_slice::<SyncState>(&pack).unwrap(), state);
        // A vault saved before the field existed opens with nobody cut off.
        let old: SyncState = serde_json::from_str(r#"{"peers":[]}"#).unwrap();
        assert!(old.cut_by.is_empty());
    }
}
