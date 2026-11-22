//! Properties of the offline transaction-hash classifier.

use proptest::prelude::*;

use super::txinfo::hash_shape;

fn hash_like() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        "[0-9a-fA-F]{60,66}",
        "0[xX][0-9a-fA-F]{62,66}",
        "[1-9A-HJ-NP-Za-km-z]{80,95}",
    ]
}

proptest! {
    #[test]
    fn hash_shape_is_total_and_ignores_surrounding_whitespace(
        raw in hash_like(),
        before in "[ \t\n]{0,3}",
        after in "[ \t\n]{0,3}",
    ) {
        let shape = hash_shape(&raw);
        prop_assert_eq!(hash_shape(raw.trim()), shape);
        prop_assert_eq!(hash_shape(&format!("{before}{raw}{after}")), shape);
    }
}
