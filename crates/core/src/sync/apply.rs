//! Applying a planned sync batch to a merged document: the pure part of
//! `apply_batch_from` (the vault side is in `service::peer_sync`).

use std::collections::BTreeMap;

use crate::model::vault::VaultDoc;
use crate::sync::plan::PlannedChange;
use crate::sync::stamp::now_ms;

/// Write the stamps and tombstones the planned changes bring into `doc.sync`.
pub(crate) fn stamp_changes(doc: &mut VaultDoc, changes: &BTreeMap<String, PlannedChange>) {
    for (key, change) in changes {
        match &change.after.stamps {
            Some(stamps) => doc.sync.stamps.insert(key.clone(), stamps.clone()),
            None => doc.sync.stamps.remove(key),
        };
        match &change.after.tombstone {
            Some(stamp) => {
                // Kept from when it arrived, whatever its own stamp says.
                doc.sync.tombstones_seen.insert(key.clone(), now_ms());
                doc.sync.tombstones.insert(key.clone(), stamp.clone())
            }
            None => {
                doc.sync.tombstones_seen.remove(key);
                doc.sync.tombstones.remove(key)
            }
        };
    }
}
