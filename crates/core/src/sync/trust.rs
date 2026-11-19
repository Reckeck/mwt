//! Trusted devices: pairing and forgetting one (docs/sync.md). Pure edits of the
//! sync bookkeeping; the stamp comes from this device's clock and the pairing
//! time from the caller.

use super::stamp::Stamp;
use super::{SyncState, TrustedPeer};
use crate::error::{Error, Result};

/// Stamps of the entries already held for `device_id` (and, when given, for
/// `public_key`): the clock must pass them before it stamps a new entry, or the
/// new one would sort before what it replaces.
pub fn known_stamps(sync: &SyncState, device_id: &str, public_key: Option<&str>) -> Vec<Stamp> {
    sync.peers
        .iter()
        .filter(|known| {
            known.device_id == device_id || public_key.is_some_and(|key| known.public_key == key)
        })
        .map(|known| known.stamp.clone())
        .collect()
}

/// A device just paired: trusted, never synced, not revoked.
pub fn new_peer(
    device_id: &str,
    name: &str,
    public_key: &str,
    paired_at: &str,
    stamp: Stamp,
) -> TrustedPeer {
    TrustedPeer {
        device_id: device_id.to_string(),
        name: name.to_string(),
        public_key: public_key.to_string(),
        paired_at: paired_at.to_string(),
        last_synced_at: None,
        revoked: None,
        stamp,
    }
}

/// Trust `peer`, replacing the entry of the same device. Paired afresh: whatever
/// the device told us before no longer holds.
pub fn pair(sync: &mut SyncState, peer: TrustedPeer) {
    let device_id = peer.device_id.clone();
    match sync
        .peers
        .iter_mut()
        .find(|known| known.device_id == device_id)
    {
        Some(known) => *known = peer,
        None => sync.peers.push(peer),
    }
    sync.cut_by.remove(&device_id);
    sync.paused.remove(&device_id);
    sync.tombstone_acks.remove(&device_id);
    sync.seen_by.remove(&device_id);
}

/// "Forget device": its key is refused here and, once this reaches them, on
/// every other device. A forgotten device pairs afresh; an old pause must not
/// block that.
pub fn revoke(sync: &mut SyncState, device_id: &str, stamp: Stamp) -> Result<()> {
    let Some(peer) = sync
        .peers
        .iter_mut()
        .find(|peer| peer.device_id == device_id)
    else {
        return Err(Error::NotFound(format!("device {device_id}")));
    };
    peer.revoked = Some(stamp.clone());
    peer.stamp = stamp;
    sync.paused.remove(device_id);
    sync.cut_by.remove(device_id);
    Ok(())
}
