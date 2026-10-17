//! A copy of the vault kept in an external document (a file in Downloads, a
//! cloud drive, a Syncthing folder) that the app cannot write atomically.
//!
//! The working vault stays local; after every save the sealed bytes go to the
//! mirror ([`VaultMirror`]). On unlock [`reconcile`] compares both copies with
//! the hash of the last copy that went through, to tell who changed.

use sha2::{Digest, Sha256};

/// Receives the vault file exactly as written after every save. Must not
/// block for long: the caller holds the vault lock.
pub trait VaultMirror: Send + Sync {
    fn push(&self, sealed: &[u8]);
}

/// What to do with a local vault and its external copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    /// Same bytes on both sides (or nothing on either).
    InSync,
    /// Only the local vault changed (or the copy is missing): write it out.
    PushLocal,
    /// Only the external copy changed (edited on another device): take it.
    PullRemote,
    /// Both changed since the last sync; the user decides.
    Conflict,
}

/// Hex SHA-256 of the file bytes.
pub fn content_hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Decide the direction of a sync. `last_synced` is the hash of the bytes
/// both sides held after the last successful push or pull.
pub fn reconcile(
    local: Option<&[u8]>,
    remote: Option<&[u8]>,
    last_synced: Option<&str>,
) -> SyncAction {
    match (local, remote) {
        (None, None) => SyncAction::InSync,
        (Some(_), None) => SyncAction::PushLocal,
        (None, Some(_)) => SyncAction::PullRemote,
        (Some(local), Some(remote)) => {
            let local = content_hash(local);
            let remote = content_hash(remote);
            if local == remote {
                return SyncAction::InSync;
            }
            match last_synced {
                Some(last) if last == local => SyncAction::PullRemote,
                Some(last) if last == remote => SyncAction::PushLocal,
                _ => SyncAction::Conflict,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_stable_hex() {
        assert_eq!(
            content_hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn reconcile_covers_every_case() {
        let old = b"old".as_slice();
        let mine = b"mine".as_slice();
        let theirs = b"theirs".as_slice();
        let last = content_hash(old);

        assert_eq!(reconcile(None, None, None), SyncAction::InSync);
        assert_eq!(
            reconcile(Some(mine), None, Some(&last)),
            SyncAction::PushLocal
        );
        assert_eq!(reconcile(None, Some(theirs), None), SyncAction::PullRemote);
        assert_eq!(reconcile(Some(mine), Some(mine), None), SyncAction::InSync);
        // Only the remote moved on.
        assert_eq!(
            reconcile(Some(old), Some(theirs), Some(&last)),
            SyncAction::PullRemote
        );
        // Only the local one moved on (the push failed or was not done yet).
        assert_eq!(
            reconcile(Some(mine), Some(old), Some(&last)),
            SyncAction::PushLocal
        );
        // Both moved on, or there is no common past.
        assert_eq!(
            reconcile(Some(mine), Some(theirs), Some(&last)),
            SyncAction::Conflict
        );
        assert_eq!(
            reconcile(Some(mine), Some(theirs), None),
            SyncAction::Conflict
        );
    }
}
