//! Tests of the search area: the old inline tests (names and bodies unchanged),
//! then the pure functions on a bare document and the delegate pins.

use super::search::*;
use crate::calendar::now_secs;
use crate::error::Error;
use crate::model::entity_ref::{EntityKind, EntityRef, Pinned};
use crate::model::store::StoreKind;
use crate::service::test_support::*;

#[test]
fn numbers_compare_without_spaces_and_commas() {
    assert_eq!(numeric_query("1 234,5").as_deref(), Some("1234.5"));
    assert_eq!(numeric_query("кофе"), None);
    assert_eq!(numeric_query("."), None);
}

#[test]
fn operations_are_found_by_comment_amount_and_wallet() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Tinkoff", StoreKind::Bank, None);
    vault
        .add_transaction(income(&card, "1000", "salary"))
        .unwrap();
    vault
        .add_transaction(payment(&card, "230", "Coffee"))
        .unwrap();
    let by_comment = vault.search_transactions("coff", &[], 10);
    assert_eq!(by_comment.len(), 1);
    let by_amount = vault.search_transactions("23", &[], 10);
    assert_eq!(by_amount.len(), 1);
    assert_eq!(vault.search_transactions("tink", &[], 10).len(), 2);
    assert!(vault.search_transactions("  ", &[], 10).is_empty());
}

#[test]
fn pins_are_deduplicated_and_valued() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    vault
        .add_transaction(income(&card, "500", "opening"))
        .unwrap();
    let wallet = Pinned::Entity {
        target: EntityRef::new(EntityKind::Store, &card),
    };
    let filter = Pinned::Filter {
        id: String::new(),
        name: " Food ".into(),
        query: "?category=groceries".into(),
    };
    vault
        .set_pinned(vec![wallet.clone(), wallet.clone(), filter])
        .unwrap();
    let pins = vault.pinned(now_secs(), "UTC");
    assert_eq!(pins.len(), 2);
    assert_eq!(pins[0].title, "Card");
    assert!(pins[0].value.is_some());
    let Pinned::Filter { id, name, query } = &pins[1].pin else {
        panic!("a filter");
    };
    assert!(!id.is_empty());
    assert_eq!(name, "Food");
    assert_eq!(query, "category=groceries");

    let gone = Pinned::Entity {
        target: EntityRef::new(EntityKind::Goal, "nope"),
    };
    vault.set_pinned(vec![gone]).unwrap();
    assert!(vault.pinned(now_secs(), "UTC")[0].missing);
    let unnamed = Pinned::Filter {
        id: String::new(),
        name: "  ".into(),
        query: String::new(),
    };
    assert!(matches!(
        vault.set_pinned(vec![unnamed]),
        Err(Error::InvalidInput(_))
    ));
}

fn doc_with_goal_and_budget() -> crate::model::vault::VaultDoc {
    use crate::model::planning::{Budget, BudgetPeriod, BudgetTarget};
    let mut doc = crate::model::vault::VaultDoc::default();
    let mut goal = crate::testkit::builders::goal("Trip", "500");
    goal.id = "g1".into();
    doc.goals.push(goal);
    doc.budgets.push(Budget {
        id: "b1".into(),
        active: false,
        target: BudgetTarget::Category("groceries".into()),
        period: BudgetPeriod::Month,
        limit: amount("100"),
        currency: "usd".into(),
        warn: true,
    });
    doc
}

#[test]
fn the_index_is_read_from_a_bare_document() {
    let doc = doc_with_goal_and_budget();
    let index: Vec<SearchEntry> = search_index(&doc)
        .into_iter()
        .filter(|found| matches!(found.target.kind, EntityKind::Goal | EntityKind::Budget))
        .collect();
    assert_eq!(index.len(), 2);
    assert_eq!(index[0].target, EntityRef::new(EntityKind::Goal, "g1"));
    assert_eq!(index[0].label, "Trip");
    assert!(!index[0].archived);
    // A budget has no name: it points at its category.
    assert_eq!(index[1].label, "");
    assert!(index[1].archived);
    assert_eq!(
        index[1].subject,
        Some(EntityRef::new(EntityKind::Category, "groceries"))
    );
    assert!(search_transactions(&doc, "trip", &[], 10).is_empty());
}

#[test]
fn pins_that_change_nothing_are_not_written() {
    let mut doc = crate::model::vault::VaultDoc::default();
    let wallet = Pinned::Entity {
        target: EntityRef::new(EntityKind::Store, "s1"),
    };
    let blank = Pinned::Entity {
        target: EntityRef::new(EntityKind::Store, " "),
    };
    let kept = plan_pins(&doc, vec![wallet.clone(), blank, wallet.clone()]).unwrap();
    assert_eq!(kept, Some(vec![wallet.clone()]));
    doc.settings.pinned = vec![wallet.clone()];
    assert_eq!(plan_pins(&doc, vec![wallet]).unwrap(), None);
}

#[test]
fn the_pins_are_capped() {
    let doc = crate::model::vault::VaultDoc::default();
    let many: Vec<Pinned> = (0..PINNED_CAP + 5)
        .map(|index| Pinned::Entity {
            target: EntityRef::new(EntityKind::Store, format!("s{index}")),
        })
        .collect();
    assert_eq!(plan_pins(&doc, many).unwrap().unwrap().len(), PINNED_CAP);
}

#[test]
fn the_delegates_equal_the_scenarios() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Tinkoff", StoreKind::Bank, None);
    vault
        .add_transaction(payment(&card, "230", "Coffee"))
        .unwrap();
    assert_eq!(vault.search_index(), search_index(&vault.doc));
    assert_eq!(
        vault.search_transactions("coff", &[], 10),
        search_transactions(&vault.doc, "coff", &[], 10)
    );
    let wallet = Pinned::Entity {
        target: EntityRef::new(EntityKind::Store, &card),
    };
    vault.set_pinned(vec![wallet.clone()]).unwrap();
    assert_eq!(vault.doc.settings.pinned, vec![wallet.clone()]);
    assert_eq!(
        vault.pinned(now_secs(), "UTC"),
        crate::usecases::household::search::pinned(&vault, now_secs(), "UTC")
    );
    crate::usecases::household::search::set_pinned(&mut vault, Vec::new()).unwrap();
    assert!(vault.doc.settings.pinned.is_empty());
}
