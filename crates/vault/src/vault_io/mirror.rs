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

/// What a push found in the external copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    /// The copy held the last synced bytes (or nothing) and now holds ours.
    Written,
    /// The copy already holds exactly these bytes.
    AlreadyThere,
    /// The copy changed on another device since the last sync: nothing was
    /// written. It is merged in when the vault is opened next.
    RemoteChanged,
}

/// Compare-and-swap push: read the copy, write only if it still holds the
/// bytes of the last sync (or does not exist). A blind write would drop
/// whatever another device saved there meanwhile.
pub fn compare_and_push<Err>(
    read_remote: impl FnOnce() -> Result<Option<Vec<u8>>, Err>,
    write_remote: impl FnOnce(&[u8]) -> Result<(), Err>,
    last_synced: Option<&str>,
    bytes: &[u8],
) -> Result<PushOutcome, Err> {
    if let Some(remote) = read_remote()? {
        let remote = content_hash(&remote);
        if remote == content_hash(bytes) {
            return Ok(PushOutcome::AlreadyThere);
        }
        if last_synced != Some(remote.as_str()) {
            return Ok(PushOutcome::RemoteChanged);
        }
    }
    write_remote(bytes)?;
    Ok(PushOutcome::Written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn push_to(remote: Option<&[u8]>, last: Option<&str>, bytes: &[u8]) -> (PushOutcome, Vec<u8>) {
        let written = RefCell::new(Vec::new());
        let outcome = compare_and_push::<()>(
            || Ok(remote.map(<[u8]>::to_vec)),
            |data| {
                written.replace(data.to_vec());
                Ok(())
            },
            last,
            bytes,
        )
        .unwrap();
        (outcome, written.into_inner())
    }

    #[test]
    fn cas_refuses_to_write_over_a_changed_remote() {
        let last = content_hash(b"old");
        let (outcome, written) = push_to(Some(b"theirs"), Some(&last), b"mine");
        assert_eq!(outcome, PushOutcome::RemoteChanged);
        assert!(written.is_empty());
        // No common past with an existing copy is a change too.
        let (outcome, written) = push_to(Some(b"theirs"), None, b"mine");
        assert_eq!(outcome, PushOutcome::RemoteChanged);
        assert!(written.is_empty());
    }

    #[test]
    fn cas_writes_over_the_last_synced_copy_or_a_missing_one() {
        let last = content_hash(b"old");
        assert_eq!(
            push_to(Some(b"old"), Some(&last), b"mine"),
            (PushOutcome::Written, b"mine".to_vec())
        );
        assert_eq!(
            push_to(None, Some(&last), b"mine"),
            (PushOutcome::Written, b"mine".to_vec())
        );
        let (outcome, written) = push_to(Some(b"mine"), None, b"mine");
        assert_eq!(outcome, PushOutcome::AlreadyThere);
        assert!(written.is_empty());
    }

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
