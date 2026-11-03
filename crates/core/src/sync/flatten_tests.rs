//! Records whose arrays and objects merge entry by entry: items added,
//! edited and removed on different devices, and the stamp of a whole field
//! written by an older device.

use serde_json::{Value, json};

use super::merge::{merge, with_legacy_heads};
use super::registry::{Collection, Rule};
use super::stamp::Stamp;
use super::state::{RecordStamps, RecordState};
use super::view::{self, split};

const LISTS: Collection = Collection {
    name: "stores",
    rule: Rule::Records,
    key: &["id"],
    flatten: &["marks"],
    flatten_by: &[("items", "id")],
    derived: &[],
    recomputed: &[],
    skip: &[],
    age: None,
    secret: false,
    secret_fields: &[],
    whole: false,
    cache: false,
};

fn at(wall: u64, device: &str) -> Stamp {
    Stamp::new(wall, 0, device)
}

fn state(value: Value, fields: &[(&str, Stamp)]) -> RecordState {
    RecordState {
        value: Some(value),
        stamps: Some(RecordStamps {
            created: at(1, "a"),
            fields: fields
                .iter()
                .map(|(name, stamp)| ((*name).to_string(), stamp.clone()))
                .collect(),
        }),
        tombstone: None,
    }
}

fn both_ways(left: &RecordState, right: &RecordState) -> RecordState {
    let there = merge(&LISTS, left, right).state;
    let back = merge(&LISTS, right, left).state;
    assert_eq!(there, back, "merge must not depend on the side");
    there
}

fn item(id: &str, text: &str) -> Value {
    json!({"id": id, "text": text})
}

fn record(items: Vec<Value>) -> Value {
    json!({"id": "r", "items": items})
}

fn items_of(merged: &RecordState) -> Vec<Value> {
    merged.value.as_ref().expect("alive")["items"]
        .as_array()
        .expect("array")
        .clone()
}

#[test]
fn array_items_merge_by_id() {
    let base = vec![item("a", "a0"), item("b", "b0")];
    let mut left_items = base.clone();
    left_items.push(item("x", "from left"));
    let mut right_items = base.clone();
    right_items.push(item("y", "from right"));
    let left = state(record(left_items), &[("items.x", at(5, "a"))]);
    let right = state(record(right_items), &[("items.y", at(6, "b"))]);
    let merged = both_ways(&left, &right);
    assert_eq!(items_of(&merged).len(), 4);

    // An edit of A on one device and of B on the other both survive.
    let left = state(
        record(vec![item("a", "a edited"), item("b", "b0")]),
        &[("items.a", at(5, "a"))],
    );
    let right = state(
        record(vec![item("a", "a0"), item("b", "b edited")]),
        &[("items.b", at(6, "b"))],
    );
    let merged = both_ways(&left, &right);
    assert_eq!(
        items_of(&merged),
        vec![item("a", "a edited"), item("b", "b edited")]
    );
}

#[test]
fn removed_item_propagates_between_new_peers() {
    let both = record(vec![item("a", "a0"), item("b", "b0")]);
    let keeper = state(both, &[("items.b", at(3, "b"))]);
    // The removal stamps the entry although its value is gone.
    let remover = state(record(vec![item("b", "b0")]), &[("items.a", at(9, "a"))]);
    let merged = both_ways(&keeper, &remover);
    assert_eq!(items_of(&merged), vec![item("b", "b0")]);
    assert!(
        merged
            .stamps
            .as_ref()
            .unwrap()
            .fields
            .contains_key("items.a")
    );
    // The merged state differs from both inputs, so the manifest sees it.
    assert_ne!(
        super::manifest::digest(&merged),
        super::manifest::digest(&keeper)
    );

    // A newer edit beats an older removal.
    let edited = state(
        record(vec![item("a", "a edited"), item("b", "b0")]),
        &[("items.a", at(12, "b"))],
    );
    let merged = both_ways(&edited, &remover);
    assert_eq!(items_of(&merged).len(), 2);

    // Everything removed: the empty array is still expressible.
    let emptied = state(
        record(vec![]),
        &[
            ("items.a", at(9, "a")),
            ("items.b", at(9, "a")),
            ("items", at(9, "a")),
        ],
    );
    let merged = both_ways(&keeper, &emptied);
    assert_eq!(items_of(&merged), Vec::<Value>::new());
}

#[test]
fn removal_stamp_survives_a_merge_where_both_sides_lack_the_item() {
    let one = state(
        record(vec![]),
        &[("items.a", at(5, "a")), ("items", at(5, "a"))],
    );
    let two = state(
        record(vec![]),
        &[("items.a", at(9, "b")), ("items", at(9, "b"))],
    );
    let merged = both_ways(&one, &two);
    assert_eq!(merged.stamps.unwrap().fields["items.a"], at(9, "b"));
}

#[test]
fn legacy_whole_field_is_a_floor() {
    let local = state(
        record(vec![
            item("a", "a new"),
            item("b", "b old"),
            item("c", "c mine"),
        ]),
        &[
            ("items.a", at(10, "a")),
            ("items.b", at(3, "a")),
            ("items.c", at(5, "a")),
        ],
    );
    // An older device: the whole array, one stamp for it.
    let old_peer = state(
        record(vec![item("a", "a theirs"), item("b", "b theirs")]),
        &[("items", at(6, "b"))],
    );
    let merged = both_ways(&local, &old_peer);
    assert_eq!(
        items_of(&merged),
        vec![
            item("a", "a new"),
            item("b", "b theirs"),
            item("c", "c mine")
        ]
    );
}

#[test]
fn legacy_floor_applies_beside_stamped_entries_of_an_object() {
    // `marks` (an object): the whole-map stamp of the older side is newer
    // than the local stamp of `a`, although the older side stamped `b`.
    let local = state(
        json!({"id": "r", "marks": {"a": "mine", "b": "mine"}}),
        &[("marks.a", at(5, "a")), ("marks.b", at(20, "a"))],
    );
    let remote = state(
        json!({"id": "r", "marks": {"a": "theirs", "b": "theirs"}}),
        &[("marks", at(9, "b")), ("marks.b", at(2, "b"))],
    );
    let merged = both_ways(&local, &remote);
    assert_eq!(
        merged.value.unwrap()["marks"],
        json!({"a": "theirs", "b": "mine"})
    );
}

#[test]
fn legacy_heads_cover_split_arrays_for_older_peers() {
    let mut stamps = RecordStamps::created(at(1, "a"));
    stamps.fields.insert("items.a".into(), at(4, "a"));
    stamps.fields.insert("items.b".into(), at(8, "a"));
    with_legacy_heads(&LISTS, &mut stamps);
    assert_eq!(stamps.fields["items"], at(8, "a"));
}

#[test]
fn join_order_is_deterministic() {
    let left = state(
        record(vec![item("a", "a"), item("x", "x"), item("m", "m")]),
        &[("items.x", at(5, "a")), ("items.m", at(7, "a"))],
    );
    let right = state(
        record(vec![item("z", "z"), item("a", "a"), item("y", "y")]),
        &[("items.z", at(6, "b")), ("items.y", at(7, "b"))],
    );
    let there = merge(&LISTS, &left, &right).state;
    let back = merge(&LISTS, &right, &left).state;
    assert_eq!(
        serde_json::to_string(&there.value).unwrap(),
        serde_json::to_string(&back.value).unwrap()
    );
    assert_eq!(items_of(&there).len(), 5);
    // Merging again changes nothing, whichever copy it starts from.
    let again = merge(&LISTS, &there, &left).state;
    assert_eq!(again.value, there.value);
}

#[test]
fn null_head_with_subfields_keeps_newest() {
    // A scalar/null whole field beside entries: the newer side wins.
    let entries = state(
        json!({"id": "r", "marks": {"a": "kept"}}),
        &[("marks.a", at(5, "a"))],
    );
    let older_null = state(json!({"id": "r", "marks": null}), &[("marks", at(3, "b"))]);
    assert_eq!(
        both_ways(&entries, &older_null).value.unwrap()["marks"],
        json!({"a": "kept"})
    );
    let newer_null = state(json!({"id": "r", "marks": null}), &[("marks", at(9, "b"))]);
    assert_eq!(
        both_ways(&entries, &newer_null).value.unwrap()["marks"],
        Value::Null
    );
}

#[test]
fn duplicate_keys_read_and_write_the_same_item() {
    // Two records under one key: `records`, `read` and `write` pick the first.
    let mut doc = json!({"stores": [
        {"id": "s", "name": "first"},
        {"id": "s", "name": "second"},
    ]});
    assert_eq!(view::records(&doc)["stores:s"]["name"], json!("first"));
    assert_eq!(
        view::read(&doc, "stores:s").unwrap()["name"],
        json!("first")
    );
    view::write(
        &mut doc,
        "stores:s",
        Some(&json!({"id": "s", "name": "new"})),
    );
    assert_eq!(doc["stores"][0]["name"], json!("new"));
    assert_eq!(doc["stores"][1]["name"], json!("second"));

    // Items of one array with a repeated key: the first one is the entry.
    let fields = split(
        &LISTS,
        &record(vec![item("a", "first"), item("a", "second")]),
    );
    assert_eq!(fields.fields["items.a"], item("a", "first"));
    assert_eq!(
        view::join(&LISTS, &fields),
        record(vec![item("a", "first")])
    );
}

#[test]
fn unkeyed_items_keep_the_array_whole() {
    let value = record(vec![item("a", "a"), json!({"text": "no id"})]);
    let fields = split(&LISTS, &value);
    assert!(fields.fields.contains_key("items"));
    assert_eq!(view::join(&LISTS, &fields), value);
}

const FUND: Collection = Collection {
    name: "emergency_fund",
    rule: Rule::Single,
    key: &[],
    flatten: &["scope"],
    flatten_by: &[("scope.store_ids", "")],
    derived: &[],
    recomputed: &[],
    skip: &[],
    age: None,
    secret: false,
    secret_fields: &[],
    whole: false,
    cache: false,
};

fn fund(ids: &[&str]) -> Value {
    json!({"scope": {"store_ids": ids, "include_children": false}, "target_months": 6})
}

fn ids_of(merged: &RecordState) -> Vec<String> {
    merged.value.as_ref().expect("alive")["scope"]["store_ids"]
        .as_array()
        .expect("array")
        .iter()
        .map(|id| id.as_str().expect("string").to_string())
        .collect()
}

fn fund_both_ways(left: &RecordState, right: &RecordState) -> RecordState {
    let there = merge(&FUND, left, right).state;
    let back = merge(&FUND, right, left).state;
    assert_eq!(there, back, "merge must not depend on the side");
    there
}

#[test]
fn a_set_of_strings_splits_one_entry_per_id() {
    let fields = split(&FUND, &fund(&["a", "b"]));
    assert_eq!(fields.fields["scope.store_ids.a"], json!("a"));
    assert_eq!(fields.fields["scope.store_ids.b"], json!("b"));
    assert!(!fields.fields.contains_key("scope.store_ids"));
    assert_eq!(view::join(&FUND, &fields), fund(&["a", "b"]));
    // Empty: the marker keeps the array alive.
    let empty = split(&FUND, &fund(&[]));
    assert_eq!(view::join(&FUND, &empty), fund(&[]));
}

#[test]
fn ids_added_on_two_devices_both_stay_and_a_removal_travels() {
    let base = ["a"];
    let left = state(fund(&["a", "x"]), &[("scope.store_ids.x", at(5, "a"))]);
    let right = state(fund(&["a", "y"]), &[("scope.store_ids.y", at(6, "b"))]);
    let merged = fund_both_ways(&left, &right);
    let mut ids = ids_of(&merged);
    ids.sort();
    assert_eq!(ids, vec!["a", "x", "y"]);
    let _ = base;

    let remover = state(fund(&[]), &[("scope.store_ids.a", at(9, "a"))]);
    let keeper = state(fund(&["a"]), &[("scope.store_ids.a", at(3, "b"))]);
    assert!(ids_of(&fund_both_ways(&keeper, &remover)).is_empty());
}

#[test]
fn a_whole_scope_stamp_of_an_older_device_is_a_floor_for_ids() {
    let local = state(
        fund(&["a", "mine"]),
        &[("scope.store_ids.mine", at(5, "a"))],
    );
    let old_peer = state(fund(&["a", "theirs"]), &[("scope", at(7, "b"))]);
    let merged = fund_both_ways(&local, &old_peer);
    let mut ids = ids_of(&merged);
    ids.sort();
    assert_eq!(ids, vec!["a", "mine", "theirs"]);
}

#[test]
fn last_item_removed_beside_a_concurrent_add() {
    // One device removes the only item (the empty array is stamped as a
    // marker), the other adds a different one.
    let remover = state(
        record(vec![]),
        &[("items.a", at(9, "a")), ("items", at(9, "a"))],
    );
    let adder = state(
        record(vec![item("a", "a0"), item("b", "b0")]),
        &[("items.b", at(12, "b"))],
    );
    let merged = both_ways(&remover, &adder);
    assert_eq!(items_of(&merged), vec![item("b", "b0")]);
}

const LINED: Collection = Collection {
    name: "transactions",
    rule: Rule::Records,
    key: &["*.meta.id"],
    flatten: &["meta"],
    flatten_by: &[(
        "lines",
        "asset.symbol+asset.coingecko_id+asset.chain+asset.contract",
    )],
    derived: &[],
    recomputed: &[("lines", "before")],
    skip: &[],
    age: None,
    secret: false,
    secret_fields: &[],
    whole: false,
    cache: false,
};

#[test]
fn a_key_path_with_plus_joins_its_parts() {
    let line = |symbol: &str, contract: Value| {
        json!({
            "asset": {"symbol": symbol, "coingecko_id": null, "chain": {"custom": "x.y"}, "contract": contract},
            "before": "1", "after": "2"
        })
    };
    let first = line("USDT", json!("0xabc.def:1"));
    let second = line("USDT", Value::Null);
    let key = |item: &Value| {
        view::item_key(
            item,
            "asset.symbol+asset.coingecko_id+asset.chain+asset.contract",
        )
    };
    assert_eq!(
        key(&first).as_deref(),
        Some(r#"USDT|null|{"custom":"x.y"}|0xabc.def:1"#)
    );
    assert_ne!(key(&first), key(&second));
    // A part that is missing altogether gives no key.
    assert_eq!(
        view::item_key(
            &json!({"asset": {"symbol": "A"}}),
            "asset.symbol+asset.nope"
        ),
        None
    );

    // The tagged record splits into one entry per asset and joins back.
    let value = json!({"adjustment": {
        "meta": {"id": "t1"}, "store_id": "s", "fee": null,
        "lines": [first.clone(), second.clone()]
    }});
    let fields = split(&LINED, &value);
    assert_eq!(fields.tag.as_deref(), Some("adjustment"));
    let entries: Vec<&String> = fields
        .fields
        .keys()
        .filter(|name| name.starts_with("lines."))
        .collect();
    assert_eq!(entries.len(), 2, "{entries:?}");
    assert!(!fields.fields.contains_key("lines"));
    assert_eq!(
        view::join_with(
            &LINED,
            &fields,
            &super::flatten::Order::new(&LINED, &[&value], &RecordStamps::default())
        ),
        value
    );
}
