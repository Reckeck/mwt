//! Which synced records are crypto-module data, that only a level-3 vault
//! may take in (H5A). The check runs on the receiving side, before a batch is
//! previewed or applied; the sending side leaves these keys out for a peer
//! below level 3 (spec E6).
//!
//! The same file answers the second question a wipe of the module raises
//! (spec E12): a device that was offline while the module was wiped still
//! holds every record of it, and the ordinary merge would hand them straight
//! back. [`newest_wipe`] reads the mark the wipe left and [`without_wiped`]
//! drops the records that mark has outlived, while anything made *after* the
//! wipe travels as usual.

use std::collections::BTreeMap;

use super::stamp::Stamp;
use super::state::{RecordStamps, RecordState};
use super::view::split_key;

/// Collections whose records are crypto-module data: every synced
/// `crypto_*` field of the document. `crypto_cursors` is not here because it
/// never travels at all (`registry::LOCAL`). The list stays here because the
/// record kinds (`kernel::record`) sit beside sync in the layer table, not
/// below it; `the_gate_agrees_with_the_record_kinds` pins the two together
/// until the kinds move below sync (core crate split, Ф6).
pub const CRYPTO_COLLECTIONS: &[&str] = &[
    "crypto_accounts",
    "crypto_events",
    "crypto_overrides",
    "crypto_tokens",
    "crypto_positions",
    "crypto_snapshots",
    "crypto_prices",
    "crypto_wiped_at",
];

#[cfg(test)]
thread_local! {
    /// Extra crypto collections for a test, beside the real set: a test of
    /// the gate itself uses an ordinary collection, so it says nothing about
    /// the crypto module's own records.
    static TEST_COLLECTIONS: std::cell::RefCell<Vec<&'static str>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Treat `names` as crypto collections on this thread until the guard drops.
#[cfg(test)]
pub fn override_for_tests(names: &[&'static str]) -> TestOverride {
    TEST_COLLECTIONS.with(|cell| cell.borrow_mut().extend_from_slice(names));
    TestOverride
}

#[cfg(test)]
pub struct TestOverride;

#[cfg(test)]
impl Drop for TestOverride {
    fn drop(&mut self) {
        TEST_COLLECTIONS.with(|cell| cell.borrow_mut().clear());
    }
}

/// Whether records of the collection `name` are crypto-module data.
pub fn is_crypto_collection(name: &str) -> bool {
    #[cfg(test)]
    if TEST_COLLECTIONS.with(|cell| cell.borrow().contains(&name)) {
        return true;
    }
    CRYPTO_COLLECTIONS.contains(&name)
}

/// Whether any of the record keys (`collection:key`) is crypto-module data.
pub fn carries_crypto_data<'key>(keys: impl IntoIterator<Item = &'key String>) -> bool {
    keys.into_iter()
        .filter_map(|key| split_key(key))
        .any(|(coll, _)| is_crypto_collection(coll.name))
}

/// The collection that carries the wipes of the module. It is crypto-module
/// data like the rest — a vault below level 3 has no business learning when
/// someone else wiped — but it is never dropped by [`without_wiped`]: it is
/// the mark itself, and a wipe must reach the other devices.
pub const WIPE_COLLECTION: &str = "crypto_wiped_at";

/// The newest wipe of the module this vault knows of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WipeMark {
    /// The sync stamp of that wipe record. A record stamped no later than
    /// this one was made before the wipe, on whichever device.
    pub stamp: Stamp,
    /// RFC 3339 of the wipe, for the one crypto collection that carries no
    /// stamp at all: `crypto_prices` unions silently, so a quote is judged
    /// by its own `ts` instead.
    pub at: String,
}

/// The newest wipe of `wipes` (`(record id, RFC 3339 of the wipe)`), by the
/// stamp sync gave that record in `stamps`. `None` while this vault knows of
/// no wipe — neither its own nor one that arrived from another device.
///
/// The stamp, not the wall clock in `at`, is what the incoming records are
/// compared against: it comes from the same hybrid logical clock they do, so
/// two devices agree on the order even when their clocks do not.
pub fn newest_wipe<'wipe>(
    wipes: impl IntoIterator<Item = (&'wipe str, &'wipe str)>,
    stamps: &BTreeMap<String, RecordStamps>,
) -> Option<WipeMark> {
    wipes
        .into_iter()
        .map(|(id, at)| WipeMark {
            stamp: stamps
                .get(&format!("{WIPE_COLLECTION}:{id}"))
                .map(|record| record.latest().clone())
                .unwrap_or_default(),
            at: at.to_string(),
        })
        .max_by(|left, right| left.stamp.cmp(&right.stamp))
}

/// `incoming` without the crypto-module records the wipe `mark` has already
/// outlived, and how many were dropped.
///
/// A record proves it belongs by being newer than the wipe: a stamp later
/// than the mark's, or — for the unstamped `crypto_prices` — a quote taken
/// after it. A crypto record that can prove neither is data from before the
/// wipe and does not come back. Records of other collections are never
/// touched here, and neither is the wipe mark itself.
pub fn without_wiped(
    incoming: BTreeMap<String, RecordState>,
    mark: &WipeMark,
) -> (BTreeMap<String, RecordState>, usize) {
    let before = incoming.len();
    let kept: BTreeMap<String, RecordState> = incoming
        .into_iter()
        .filter(|(key, state)| !outlived(key, state, mark))
        .collect();
    let dropped = before - kept.len();
    (kept, dropped)
}

/// Whether this one incoming record is crypto-module data from before the
/// wipe.
fn outlived(key: &str, state: &RecordState, mark: &WipeMark) -> bool {
    let Some((coll, _)) = split_key(key) else {
        return false;
    };
    if !is_crypto_collection(coll.name) || coll.name == WIPE_COLLECTION {
        return false;
    }
    match state.latest() {
        Some(latest) => *latest <= mark.stamp,
        // No stamp at all: a union (`crypto_prices`). Its own age field says
        // when it was taken; without a readable one it cannot prove it
        // postdates the wipe.
        None => !taken_after(coll.age, state, &mark.at),
    }
}

/// Whether an unstamped record's age field reads as a time after `at`.
fn taken_after(age: Option<&str>, state: &RecordState, at: &str) -> bool {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    let Some((field, value)) = age.zip(state.value.as_ref()) else {
        return false;
    };
    let Some(taken) = value.get(field).and_then(serde_json::Value::as_str) else {
        return false;
    };
    match (
        OffsetDateTime::parse(taken, &Rfc3339),
        OffsetDateTime::parse(at, &Rfc3339),
    ) {
        (Ok(taken), Ok(wiped)) => taken > wiped,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every synced `crypto_*` field of the document is behind the gate, and
    /// nothing else is: a field added to the module later must be listed, or
    /// it would reach a vault below level 3.
    #[test]
    fn every_synced_crypto_collection_is_behind_the_gate() {
        let doc = serde_json::to_value(crate::VaultDoc::default()).expect("doc");
        let synced: Vec<String> = doc
            .as_object()
            .expect("object")
            .keys()
            .filter(|name| crate::kernel::record::level3_collections().any(|known| known == *name))
            .filter(|name| !crate::sync::registry::LOCAL.contains(&name.as_str()))
            .cloned()
            .collect();
        for name in &synced {
            assert!(
                is_crypto_collection(name),
                "VaultDoc.{name} syncs but is not in CRYPTO_COLLECTIONS"
            );
        }
        for name in CRYPTO_COLLECTIONS {
            assert!(
                synced.iter().any(|field| field == name),
                "CRYPTO_COLLECTIONS names {name}, which is not a synced field"
            );
        }
        assert!(carries_crypto_data(&vec![
            "crypto_events:cev-1".to_string()
        ]));
        assert!(!carries_crypto_data(&vec!["stores:abc".to_string()]));
    }

    /// `CRYPTO_COLLECTIONS` is exactly the level-3 collections of the record
    /// kinds less the device-local ones.
    #[test]
    fn the_gate_agrees_with_the_record_kinds() {
        let mut from_kinds: Vec<&str> = crate::kernel::record::level3_collections()
            .filter(|name| !crate::sync::registry::LOCAL.contains(name))
            .collect();
        let mut listed = CRYPTO_COLLECTIONS.to_vec();
        from_kinds.sort_unstable();
        listed.sort_unstable();
        assert_eq!(listed, from_kinds);
    }

    /// The gate answers exactly what the old `crypto_` prefix rule did, for
    /// every record kind and every collection of the sync registry.
    #[test]
    fn the_gate_agrees_with_the_old_prefix_rule() {
        let old_rule = |name: &str| {
            name.starts_with("crypto_") && !crate::sync::registry::LOCAL.contains(&name)
        };
        let names = crate::kernel::record::record_kinds()
            .iter()
            .map(|kind| kind.collection)
            .chain(
                crate::sync::registry::COLLECTIONS
                    .iter()
                    .map(|coll| coll.name),
            );
        for name in names {
            assert_eq!(is_crypto_collection(name), old_rule(name), "{name}");
        }
    }

    #[test]
    fn an_override_marks_a_collection_until_it_drops() {
        let keys = vec!["stores:abc".to_string()];
        {
            let _guard = override_for_tests(&["stores"]);
            assert!(carries_crypto_data(&keys));
        }
        assert!(!carries_crypto_data(&keys));
    }
}
