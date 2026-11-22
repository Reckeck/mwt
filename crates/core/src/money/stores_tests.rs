//! Tests of the stores area: the tree rules on a bare `VaultDoc`, and the
//! scenarios against the delegates they replace.

use super::stores::*;
use crate::error::{Error, StoreTreeViolation, WalletRule};
use crate::model::store::{Store, StoreKind};
use crate::model::vault::VaultDoc;

fn doc_with(stores: Vec<Store>) -> VaultDoc {
    VaultDoc {
        stores,
        ..VaultDoc::default()
    }
}

fn wallet(id: &str, name: &str, parent: Option<&str>) -> Store {
    let mut store = Store::new(name, StoreKind::Bank, "#000");
    store.id = id.into();
    store.parent_id = parent.map(str::to_string);
    store
}

#[test]
fn a_name_taken_among_siblings_is_refused_ignoring_case() {
    let doc = doc_with(vec![wallet("a", "Main", None)]);
    let taken = validate_placement(&doc, None, None, "  main ");
    assert!(matches!(
        taken,
        Err(Error::StoreTree(StoreTreeViolation::NameTaken))
    ));
    assert!(validate_placement(&doc, Some("a"), None, "Main").is_ok());
}

#[test]
fn a_node_cannot_move_under_its_own_subtree() {
    let doc = doc_with(vec![
        wallet("root", "Root", None),
        wallet("child", "Child", Some("root")),
    ]);
    let cycle = validate_placement(&doc, Some("root"), Some("child"), "Root");
    assert!(matches!(cycle, Err(Error::StoreTree(_))));
}

#[test]
fn a_free_name_gets_a_counter_among_siblings() {
    let doc = doc_with(vec![
        wallet("a", "Main", None),
        wallet("b", "Main (2)", None),
    ]);
    assert_eq!(free_name(&doc, "x", None, "Main"), "Main (3)");
    assert_eq!(free_name(&doc, "x", None, "Other"), "Other");
}

#[test]
fn extras_that_do_not_fit_the_kind_are_refused() {
    let doc = doc_with(Vec::new());
    let claims = ExtraClaims {
        custody: true,
        ..ExtraClaims::default()
    };
    let refused = check_store_extras(&doc, StoreKind::Bank, None, true, &claims);
    assert!(matches!(
        refused,
        Err(Error::WalletRule(WalletRule::NotApplicable))
    ));
    assert!(check_store_extras(&doc, StoreKind::Crypto, None, true, &claims).is_ok());
}

#[test]
fn archiving_reaches_the_subtree_and_a_wallet_with_a_child_cannot_be_deleted() {
    let doc = doc_with(vec![
        wallet("root", "Root", None),
        wallet("child", "Child", Some("root")),
    ]);
    let mut affected = plan_archive(&doc, "root", true).unwrap();
    affected.sort();
    assert_eq!(affected, ["child", "root"]);
    assert!(check_deletable(&doc, "root").is_err());
    assert!(check_deletable(&doc, "child").is_ok());
    assert!(matches!(
        check_deletable(&doc, "nope"),
        Err(Error::Missing(_))
    ));
}

#[test]
fn moving_to_the_same_parent_is_a_stay() {
    let doc = doc_with(vec![
        wallet("root", "Root", None),
        wallet("child", "Child", Some("root")),
    ]);
    assert!(matches!(
        plan_move(&doc, "child", Some("root")),
        Ok(StoreMove::Stay(_))
    ));
}
