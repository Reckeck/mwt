//! Tests of the deterministic record ids (`sync::sync_ids`).

use super::sync_ids::*;

#[test]
fn a_plain_key_is_kept_as_it_is() {
    assert_eq!(sync_safe_id(&["abc-1", "USD"]), "abc-1_USD");
}

#[test]
fn colons_and_pipes_are_replaced_and_hashed() {
    let id = sync_safe_id(&["asset:BTC|bitcoin", "2026-01-01"]);
    assert!(id.starts_with("asset_BTC_bitcoin_2026-01-01-"), "{id}");
    assert!(id.chars().all(super::sync_ids::allowed), "{id}");
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
