//! What this vault tells another device about its records, and what it makes of a batch
//! before applying it (docs/sync.md). Pure: every function reads the document and the
//! serialized records it is given; stamping the pending edits and building the records
//! happen in the scenarios (`usecases::sync::read`).

use std::collections::BTreeMap;

use serde_json::Value;

use super::manifest::{Manifest, digest};
use super::plan::plan_seen;
use super::registry::Rule;
use super::view::{self, record_key, split_key};
use super::{RecordState, SyncSummary};
use crate::crypto::level::SecurityLevel;
use crate::error::{Error, Result};
use crate::model::vault::{VaultDoc, iso_to_millis};

/// Whether a device protected at `level` may hold the crypto module's records
/// (only level 3 may, spec E0).
pub(crate) fn may_hold_crypto(level: SecurityLevel) -> bool {
    level == SecurityLevel::Crypto
}

/// Whether the record key (`collection:key`) belongs to the crypto module.
pub(crate) fn is_crypto_key(key: &str) -> bool {
    split_key(key).is_some_and(|(coll, _)| super::crypto_gate::is_crypto_collection(coll.name))
}

/// The state of one record: its value and stamps, or its tombstone when it is gone.
pub(crate) fn record_state(
    doc: &VaultDoc,
    records: &BTreeMap<String, Value>,
    key: &str,
) -> RecordState {
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
            stamps: Some(doc.sync.stamps.get(key).cloned().unwrap_or_default()),
            tombstone: None,
        },
        None => RecordState {
            value: None,
            stamps: None,
            tombstone: doc.sync.tombstones.get(key).cloned(),
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

/// Whether a cache point is worth syncing here: rates of a currency that
/// is not in use are thinned to one point right after a merge (the rule
/// of `thin_rate_caches`), so asking for or offering them only repeats
/// the same list in every session. Other keys always qualify.
pub(crate) fn wants_cache_key(key: &str, keeps_fx: &impl Fn(&str) -> bool) -> bool {
    match key.strip_prefix("fx_cache:") {
        Some(point) => keeps_fx(point.split_once('|').map_or(point, |(code, _)| code)),
        None => true,
    }
}

/// One digest per record and deletion, for a peer protected at `peer`: below
/// level 3 the crypto module's records are left out, so their keys never reach
/// the network. `union_since_ms` leaves out cached prices and rates older than
/// that (a first sync sends 90 days). Also says how many records were left out.
pub fn manifest_for(
    doc: &VaultDoc,
    records: &BTreeMap<String, Value>,
    keeps_fx: &impl Fn(&str) -> bool,
    union_since_ms: Option<u64>,
    peer: SecurityLevel,
) -> (Manifest, usize) {
    let withhold = !may_hold_crypto(peer);
    let mut withheld = 0;
    let mut manifest = Manifest::new();
    for key in records.keys() {
        if !wants_cache_key(key, keeps_fx) {
            continue;
        }
        if withhold && is_crypto_key(key) {
            withheld += 1;
            continue;
        }
        if let (Some(since), Some((coll, _))) = (union_since_ms, split_key(key))
            && let Some(field) = coll.age
            && let Some(Value::String(at)) = records[key].get(field)
            && u64::try_from(iso_to_millis(at)).unwrap_or(0) < since
        {
            continue;
        }
        manifest.insert(
            key.clone(),
            digest(&shared_state(record_state(doc, records, key), key)),
        );
    }
    for key in doc.sync.tombstones.keys() {
        if !records.contains_key(key) {
            if withhold && is_crypto_key(key) {
                withheld += 1;
                continue;
            }
            manifest.insert(
                key.clone(),
                digest(&shared_state(record_state(doc, records, key), key)),
            );
        }
    }
    (manifest, withheld)
}

/// `(to_request, to_send)` for two manifests: [`super::manifest::diff`] without the
/// cache points this device would thin away as soon as they arrive.
pub fn diff_wanted(
    mine: &Manifest,
    theirs: &Manifest,
    keeps_fx: &impl Fn(&str) -> bool,
) -> (Vec<String>, Vec<String>) {
    let (mut request, send) = super::manifest::diff(mine, theirs);
    request.retain(|key| wants_cache_key(key, keeps_fx));
    (request, send)
}

/// The full state of each asked record (absent keys come back empty), for a peer
/// protected at `peer`: below level 3 a crypto record is not answered even when
/// asked for (a broken or hostile peer may ask for any key), so it never leaves
/// this device.
pub fn records_for(
    doc: &VaultDoc,
    records: &BTreeMap<String, Value>,
    keys: &[String],
    peer: SecurityLevel,
) -> BTreeMap<String, RecordState> {
    let withhold = !may_hold_crypto(peer);
    keys.iter()
        .filter(|key| split_key(key).is_some())
        .filter(|key| !(withhold && is_crypto_key(key)))
        .map(|key| {
            let mut state = record_state(doc, records, key);
            if let (Some((coll, _)), Some(stamps)) = (split_key(key), state.stamps.as_mut()) {
                super::merge::with_legacy_heads(coll, stamps);
            }
            (key.clone(), state)
        })
        .collect()
}

/// Refuse records filed under a key they do not carry, or under a
/// collection that does not sync: a broken or hostile peer must not be
/// able to overwrite one record with another.
pub fn check_incoming(incoming: &BTreeMap<String, RecordState>) -> Result<()> {
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

/// What applying `incoming` from the device `device_id` would change: what that
/// device has already seen of this one is not listed as overwritten.
pub fn preview_summary(
    doc: &VaultDoc,
    records: &BTreeMap<String, Value>,
    incoming: &BTreeMap<String, RecordState>,
    device_id: &str,
) -> SyncSummary {
    let seen = doc.sync.seen_by.get(device_id);
    plan_seen(incoming, |key| record_state(doc, records, key), seen).summary
}
