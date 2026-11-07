//! Which synced records are crypto-module data, that only a level-3 vault
//! may take in (H5A). Stage 1 has no such collection yet: the set is empty
//! and E0 fills it. The check runs on the receiving side, before a batch is
//! previewed or applied.

use super::view::split_key;

/// Collections whose records are crypto-module data. Empty in stage 1.
pub const CRYPTO_COLLECTIONS: &[&str] = &[];

#[cfg(test)]
thread_local! {
    /// Extra crypto collections for a test (the real set is still empty).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_one_has_no_crypto_collection() {
        assert!(CRYPTO_COLLECTIONS.is_empty());
        let keys = vec!["stores:abc".to_string()];
        assert!(!carries_crypto_data(&keys));
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
