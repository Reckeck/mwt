//! Scenarios of the trusted devices: each takes a stamp from this device's clock, then ends
//! in one `vault.transact` over the pure edit in `sync::trust`.

use crate::error::Result;
use crate::model::vault::now_iso;
use crate::service::UnlockedVault;
use crate::sync::trust::{new_peer, pair, revoke};

/// Trust a device after both sides confirmed the pairing code.
pub fn sync_trust_peer(
    vault: &mut UnlockedVault,
    device_id: &str,
    name: &str,
    public_key: &str,
) -> Result<()> {
    // The new entry must sort after whatever is known of this device/key.
    let stamp = vault.stamp_past_known_peer(device_id, Some(public_key));
    let peer = new_peer(device_id, name, public_key, &now_iso(), stamp);
    vault.transact(|tx| {
        pair(&mut tx.doc_mut().sync, peer);
        Ok(())
    })
}

/// "Forget device": its key is refused here and, once this reaches
/// them, on every other device.
pub fn sync_forget_peer(vault: &mut UnlockedVault, device_id: &str) -> Result<()> {
    let stamp = vault.stamp_past_known_peer(device_id, None);
    vault.transact(|tx| revoke(&mut tx.doc_mut().sync, device_id, stamp))
}
