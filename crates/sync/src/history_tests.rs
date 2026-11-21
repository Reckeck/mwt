//! Tests of the store-change diff.

use super::*;

fn amount(text: &str) -> Amount {
    Amount::parse(text).unwrap()
}

fn values(entries: &[(&str, &str)]) -> HashMap<String, Amount> {
    entries
        .iter()
        .map(|(id, value)| (id.to_string(), amount(value)))
        .collect()
}

#[test]
fn store_changes_lists_moved_stores_sorted_by_name_and_skips_unchanged() {
    let before = values(&[("a", "10"), ("b", "5"), ("same", "7")]);
    let after = values(&[("a", "12"), ("c", "3"), ("same", "7")]);
    let names = HashMap::from([("a", "Zeta"), ("b", "Beta"), ("c", "Alpha"), ("same", "S")]);
    let one = amount("1");
    let changes = store_changes(
        &StoreValues {
            usd: &before,
            fx: one,
        },
        &StoreValues {
            usd: &after,
            fx: one,
        },
        &names,
    );
    let ids: Vec<&str> = changes.iter().map(|change| change.id.as_str()).collect();
    assert_eq!(ids, ["c", "b", "a"]);
    assert_eq!(changes[0].before, amount("0"));
    assert_eq!(changes[0].after, amount("3"));
    assert_eq!(changes[1].after, amount("0"));
    assert_eq!(changes[2].before, amount("10"));
    assert_eq!(changes[2].after, amount("12"));
}

#[test]
fn store_changes_applies_each_sides_rate() {
    let usd = values(&[("a", "10")]);
    let changes = store_changes(
        &StoreValues {
            usd: &usd,
            fx: amount("1"),
        },
        &StoreValues {
            usd: &usd,
            fx: amount("2"),
        },
        &HashMap::new(),
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].before, amount("10"));
    assert_eq!(changes[0].after, amount("20"));
    assert_eq!(changes[0].name, "");
}
