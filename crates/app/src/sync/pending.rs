//! The device a received vault copy must trust at its first unlock, kept in
//! the app settings so a process kill in between does not lose it, and the
//! trust bookkeeping around a vault hand-over.

use std::path::Path;

use trade_core::appconfig::{AppSettings, PendingTrustPref};
use trade_core::error::Result;
use trade_core::model::notify::NoticeRecord;
use trade_core::service::UnlockedVault;

use super::transfer::Delivered;

/// How long a received copy waits for its first unlock.
pub(super) const PENDING_TTL_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// Remember a device as the one `path` must trust once it is unlocked.
pub(super) fn remember(
    settings: &mut AppSettings,
    path: &Path,
    device_id: &str,
    device_name: &str,
    device_key: &str,
    now_ms: u64,
) {
    let path = path.to_string_lossy().into_owned();
    settings
        .pending_trusts
        .retain(|known| !(known.path == path && known.device_id == device_id));
    settings.pending_trusts.push(PendingTrustPref {
        path,
        device_id: device_id.to_string(),
        device_name: device_name.to_string(),
        device_key: device_key.to_string(),
        expires_ms: now_ms.saturating_add(PENDING_TTL_MS),
    });
}

/// The entries waiting for the vault at `path` (removed from the settings);
/// expired entries of any file are dropped on the way.
pub(super) fn take(settings: &mut AppSettings, path: &Path, now_ms: u64) -> Vec<PendingTrustPref> {
    let path = path.to_string_lossy();
    settings
        .pending_trusts
        .retain(|known| known.expires_ms > now_ms);
    let (mine, rest) = std::mem::take(&mut settings.pending_trusts)
        .into_iter()
        .partition(|known| known.path == path);
    settings.pending_trusts = rest;
    mine
}

/// Trust the sender in the freshly opened received copy.
pub(super) fn apply(vault: &mut UnlockedVault, pending: &PendingTrustPref) -> Result<()> {
    vault.sync_trust_peer(
        &pending.device_id,
        &pending.device_name,
        &pending.device_key,
    )?;
    // A new transfer is a fresh start: an old decline must not keep it out.
    vault.sync_resume_peer(&pending.device_id)?;
    vault.sync_clear_cut(&pending.device_id)?;
    Ok(())
}

/// Whether the vault open now is the file that was sent: only then is the
/// delivery written into it (a locked or another vault gets nothing).
pub(super) fn is_sent_vault(open: &Path, sent: &Path) -> bool {
    open == sent
}

/// The vault went to `delivered`: record it, trust the device and lift
/// whatever kept it out (a pause, a cut) so the new copy links at once.
pub(super) fn record_delivery(
    vault: &mut UnlockedVault,
    delivered: &Delivered,
    ru: bool,
) -> Result<NoticeRecord> {
    let notice = vault.sync_record_transfer(
        &delivered.device_id,
        &delivered.device_name,
        &delivered.device_key,
        ru,
    )?;
    vault.sync_resume_peer(&delivered.device_id)?;
    vault.sync_clear_cut(&delivered.device_id)?;
    Ok(notice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delivery_is_recorded_only_in_the_vault_that_was_sent() {
        let sent = Path::new("/vaults/home.wallet");
        assert!(is_sent_vault(Path::new("/vaults/home.wallet"), sent));
        assert!(!is_sent_vault(Path::new("/vaults/work.wallet"), sent));
    }
}
