//! Deterministic ids of the records that have a natural key (stage M).
//!
//! Two devices that create "the same" record (debt terms of one contact in one
//! asset, the reading of one meter on one day) give it the same id, so the
//! merge sees one record instead of two. Records made by an older build keep
//! their random ids; the duplicate pass of `peer_sync::repair` folds them in.

use blake2::{Blake2s256, Digest};

const SEPARATOR: char = '_';
const HASH_BYTES: usize = 6;

pub(super) fn allowed(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')
}

/// Parts of a key as one id-safe string: every char outside `[A-Za-z0-9._-]`
/// becomes `_`, the parts are joined by `_`. When anything was replaced (or a
/// part holds the separator itself), a short blake2 hash of the original parts
/// is appended, so two different keys never come out the same.
pub fn sync_safe_id(parts: &[&str]) -> String {
    let mut lossy = false;
    let cleaned: Vec<String> = parts
        .iter()
        .map(|part| {
            part.chars()
                .map(|ch| {
                    if allowed(ch) {
                        ch
                    } else {
                        lossy = true;
                        '_'
                    }
                })
                .collect::<String>()
        })
        .collect();
    if parts.len() > 1 && parts.iter().any(|part| part.contains(SEPARATOR)) {
        lossy = true;
    }
    let mut id = cleaned.join(&SEPARATOR.to_string());
    if lossy {
        let mut hasher = Blake2s256::new();
        for part in parts {
            // Length-prefixed, so the split points are part of the input.
            hasher.update((part.len() as u64).to_le_bytes());
            hasher.update(part.as_bytes());
        }
        let digest = hasher.finalize();
        id.push('-');
        for byte in digest.iter().take(HASH_BYTES) {
            id.push_str(&format!("{byte:02x}"));
        }
    }
    id
}

/// Debt terms of one contact in one asset.
pub fn debt_terms_id(contact_id: &str, asset: &str) -> String {
    format!("debt-{}", sync_safe_id(&[contact_id, asset]))
}

/// The reading of one meter (and zone) on one day: `when` is the reading's
/// `sort_key()` (month, day).
pub fn reading_id(item_id: &str, zone: Option<&str>, when: &(String, u8)) -> String {
    let day = when.1.to_string();
    format!(
        "mr-{}",
        sync_safe_id(&[item_id, zone.unwrap_or("-"), &when.0, &day])
    )
}

/// The valuation of one asset on one day.
pub fn valuation_id(asset_key: &str, date: &str) -> String {
    format!("val-{}", sync_safe_id(&[asset_key, date]))
}

/// The envelope that holds a goal's money in one wallet and asset.
pub fn goal_envelope_id(goal_id: &str, store_id: &str, asset_key: &str) -> String {
    format!("goal-{}", sync_safe_id(&[goal_id, store_id, asset_key]))
}

/// The contact a shared-event guest became (one per event and participant).
pub fn guest_contact_id(event_id: &str, participant_id: &str) -> String {
    format!("guest-{}", sync_safe_id(&[event_id, participant_id]))
}

/// The purchase that links one vault payment into an event.
pub fn payment_link_id(tx_id: &str) -> String {
    format!("link-{}", sync_safe_id(&[tx_id]))
}

/// A user asset class named `name` (case-insensitive, trimmed).
pub fn asset_class_id(name: &str) -> String {
    format!("cls-{}", sync_safe_id(&[&name.trim().to_lowercase()]))
}
