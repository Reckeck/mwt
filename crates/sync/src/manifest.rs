//! The first message of a sync: one short digest per record, so the two
//! devices exchange only the records that differ.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::state::RecordState;

/// `collection:key` → digest of everything sync knows about the record.
pub type Manifest = BTreeMap<String, u64>;

/// A digest that is the same on every device and app version for the same
/// state: SHA-256 of the canonical JSON (sorted keys), first 8 bytes.
pub fn digest(state: &RecordState) -> u64 {
    let hash = Sha256::digest(serde_json::to_string(state).unwrap_or_default().as_bytes());
    let mut head = [0u8; 8];
    head.copy_from_slice(&hash[..8]);
    u64::from_le_bytes(head)
}

/// What each side must send: records the other side lacks or holds in
/// another state. `(to_request, to_send)` from this side's point of view.
pub fn diff(mine: &Manifest, theirs: &Manifest) -> (Vec<String>, Vec<String>) {
    let request = theirs
        .iter()
        .filter(|(key, digest)| mine.get(*key) != Some(digest))
        .map(|(key, _)| key.clone())
        .collect();
    let send = mine
        .iter()
        .filter(|(key, digest)| theirs.get(*key) != Some(digest))
        .map(|(key, _)| key.clone())
        .collect();
    (request, send)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_differing_keys_travel() {
        let mine: Manifest = [("a".into(), 1), ("b".into(), 2)].into();
        let theirs: Manifest = [("b".into(), 3), ("c".into(), 4)].into();
        let (request, send) = diff(&mine, &theirs);
        assert_eq!(request, vec!["b", "c"]);
        assert_eq!(send, vec!["a", "b"]);
    }
}
