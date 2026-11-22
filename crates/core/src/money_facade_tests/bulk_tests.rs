//! Tests of the bulk area: several operations at once, all or nothing.

use crate::error::Error;
use crate::model::vault::VaultDoc;
use crate::money::bulk::*;
use crate::service::test_support::*;

// -- the pure parts, on a bare document ---------------------------------------

#[test]
fn tx_index_finds_an_operation_in_a_bare_document() {
    let tx = payment("card", "5", "a");
    let id = tx.meta().id.clone();
    let doc = VaultDoc {
        transactions: vec![income("card", "9", "b"), tx],
        ..VaultDoc::default()
    };
    assert_eq!(tx_index(&doc, &id).unwrap(), 1);
    assert!(matches!(tx_index(&doc, "nope"), Err(Error::Missing(_))));
}

#[test]
fn with_categories_returns_the_new_operation_and_the_old_categories() {
    let tx = payment("card", "5", "a");
    let change = TxCategories {
        id: tx.meta().id.clone(),
        categories: vec!["taxi".into()],
    };
    let (changed, had) = with_categories(&tx, change);
    assert_eq!(changed.meta().categories, vec!["taxi".to_string()]);
    assert_eq!(had.categories, vec!["groceries".to_string()]);
    assert_eq!(
        tx.meta().categories,
        vec!["groceries".to_string()],
        "the input is not touched"
    );
}

#[test]
fn with_store_moves_a_payment_and_refuses_a_transfer() {
    let tx = payment("card", "5", "a");
    let change = TxStore {
        id: tx.meta().id.clone(),
        store_id: "cash".into(),
    };
    let (moved, had) = with_store(&tx, change).unwrap();
    assert_eq!(moved.legs()[0].store_id, "cash");
    assert_eq!(had.store_id, "card");
    let two_sided = transfer("card", "cash", "5");
    let refused = TxStore {
        id: two_sided.meta().id.clone(),
        store_id: "x".into(),
    };
    assert!(with_store(&two_sided, refused).is_err());
}
