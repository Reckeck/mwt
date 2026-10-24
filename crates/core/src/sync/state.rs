//! What a vault keeps about sync: change stamps, deletions, trusted devices
//! and the batches it took in (for undo). Stored in [`crate::VaultDoc::sync`].

use std::collections::BTreeMap;

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
    /// Set by «Забыть устройство» on any device; a revoked key is refused everywhere.
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
    /// The latest stamp anywhere, to start the clock after it.
    pub fn latest_stamp(&self) -> Option<&Stamp> {
        self.stamps
            .values()
            .map(RecordStamps::latest)
            .chain(self.tombstones.values())
            .max()
    }

    /// A trusted, not revoked peer by its key.
    pub fn trusted(&self, public_key: &str) -> Option<&TrustedPeer> {
        self.peers
            .iter()
            .find(|peer| peer.public_key == public_key && peer.revoked.is_none())
    }

    /// Whether this key was trusted once and then forgotten.
    pub fn is_revoked(&self, public_key: &str) -> bool {
        self.peers
            .iter()
            .any(|peer| peer.public_key == public_key && peer.revoked.is_some())
    }

    /// Take in another device's list: per device, the later entry wins.
    pub fn merge_peers(&mut self, remote: &[TrustedPeer]) -> bool {
        let mut changed = false;
        for theirs in remote {
            match self
                .peers
                .iter_mut()
                .find(|mine| mine.device_id == theirs.device_id)
            {
                Some(mine) if theirs.stamp > mine.stamp => {
                    *mine = theirs.clone();
                    changed = true;
                }
                Some(_) => {}
                None => {
                    self.peers.push(theirs.clone());
                    changed = true;
                }
            }
        }
        changed
    }
}
