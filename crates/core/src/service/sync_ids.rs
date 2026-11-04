//! Deterministic ids of the records that have a natural key (stage M).
//!
//! Two devices that create "the same" record (debt terms of one contact in one
//! asset, the reading of one meter on one day) give it the same id, so the
//! merge sees one record instead of two. Records made by an older build keep
//! their random ids; the duplicate pass of `peer_sync::repair` folds them in.

use blake2::{Blake2s256, Digest};

const SEPARATOR: char = '_';
const HASH_BYTES: usize = 6;

fn allowed(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')
}

/// Parts of a key as one id-safe string: every char outside `[A-Za-z0-9._-]`
/// becomes `_`, the parts are joined by `_`. When anything was replaced (or a
/// part holds the separator itself), a short blake2 hash of the original parts
/// is appended, so two different keys never come out the same.
pub(crate) fn sync_safe_id(parts: &[&str]) -> String {
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
pub(crate) fn debt_terms_id(contact_id: &str, asset: &str) -> String {
    format!("debt-{}", sync_safe_id(&[contact_id, asset]))
}

/// The reading of one meter (and zone) on one day: `when` is the reading's
/// `sort_key()` (month, day).
pub(crate) fn reading_id(item_id: &str, zone: Option<&str>, when: &(String, u8)) -> String {
    let day = when.1.to_string();
    format!(
        "mr-{}",
        sync_safe_id(&[item_id, zone.unwrap_or("-"), &when.0, &day])
    )
}

/// The valuation of one asset on one day.
pub(crate) fn valuation_id(asset_key: &str, date: &str) -> String {
    format!("val-{}", sync_safe_id(&[asset_key, date]))
}

/// The envelope that holds a goal's money in one wallet and asset.
pub(crate) fn goal_envelope_id(goal_id: &str, store_id: &str, asset_key: &str) -> String {
    format!("goal-{}", sync_safe_id(&[goal_id, store_id, asset_key]))
}

/// The contact a shared-event guest became (one per event and participant).
pub(crate) fn guest_contact_id(event_id: &str, participant_id: &str) -> String {
    format!("guest-{}", sync_safe_id(&[event_id, participant_id]))
}

/// The purchase that links one vault payment into an event.
pub(crate) fn payment_link_id(tx_id: &str) -> String {
    format!("link-{}", sync_safe_id(&[tx_id]))
}

/// A user asset class named `name` (case-insensitive, trimmed).
pub(crate) fn asset_class_id(name: &str) -> String {
    format!("cls-{}", sync_safe_id(&[&name.trim().to_lowercase()]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_key_is_kept_as_it_is() {
        assert_eq!(sync_safe_id(&["abc-1", "USD"]), "abc-1_USD");
    }

    #[test]
    fn colons_and_pipes_are_replaced_and_hashed() {
        let id = sync_safe_id(&["asset:BTC|bitcoin", "2026-01-01"]);
        assert!(id.starts_with("asset_BTC_bitcoin_2026-01-01-"), "{id}");
        assert!(id.chars().all(allowed), "{id}");
    }

    #[test]
    fn keys_that_clean_alike_do_not_collide() {
        let colon = sync_safe_id(&["a:b"]);
        let pipe = sync_safe_id(&["a|b"]);
        let plain = sync_safe_id(&["a_b"]);
        assert_ne!(colon, pipe);
        assert_ne!(colon, plain);
        assert_ne!(pipe, plain);
    }

    #[test]
    fn the_same_text_split_differently_does_not_collide() {
        assert_ne!(sync_safe_id(&["a_b", "c"]), sync_safe_id(&["a", "b_c"]));
        assert_ne!(sync_safe_id(&["a:b", "c"]), sync_safe_id(&["a", "b:c"]));
    }

    #[test]
    fn the_id_does_not_depend_on_the_device() {
        assert_eq!(sync_safe_id(&["x:y"]), sync_safe_id(&["x:y"]));
        assert_eq!(
            valuation_id("coingecko:bitcoin", "2026-01-01"),
            valuation_id("coingecko:bitcoin", "2026-01-01")
        );
    }

    #[test]
    fn the_scheme_prefixes_tell_the_records_apart() {
        assert_eq!(debt_terms_id("c1", "USD"), "debt-c1_USD");
        assert_eq!(
            reading_id("m1", None, &("2026-03".into(), 5)),
            "mr-m1_-_2026-03_5"
        );
        assert!(goal_envelope_id("g", "s", "USD").starts_with("goal-"));
        assert_eq!(asset_class_id("  Bonds "), asset_class_id("bonds"));
        assert_eq!(guest_contact_id("e1", "p1"), "guest-e1_p1");
        assert_eq!(payment_link_id("t1"), "link-t1");
    }
}
