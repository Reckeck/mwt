//! Tests of the categories area that need the vault (`UnlockedVault`): moved from
//! `money/categories_tests.rs` with their assertions unchanged (crate-split T13).

use crate::error::Error;
use crate::model::VaultDoc;
use crate::model::category::CategoryGroup;
use crate::money::categories::*;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::usecases::household::categories as scenarios;

#[test]
fn a_new_vault_has_the_default_categories() {
    let vault = UnlockedVault::in_memory(VaultDoc::default());
    let group = |key: &str| {
        vault
            .doc
            .categories
            .iter()
            .find(|category| category.key == key)
            .map(|category| category.group)
    };
    assert_eq!(group("salary"), Some(CategoryGroup::Income));
    assert_eq!(group("rent_mortgage"), Some(CategoryGroup::Fixed));
    assert_eq!(group("taxi"), Some(CategoryGroup::Daily));
    assert_eq!(group("savings"), Some(CategoryGroup::Savings));
    assert_eq!(group("trading"), Some(CategoryGroup::Service));
}

#[test]
fn renaming_keeps_transactions_and_deleting_needs_it_unused() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let coffee = vault
        .create_category(None, "Coffee", CategoryGroup::Daily)
        .unwrap();
    assert_eq!(coffee.key, "coffee");
    let again = vault
        .create_category(None, "coffee", CategoryGroup::Daily)
        .unwrap();
    assert_eq!(again.key, "coffee_2");

    let mut tx = payment(&store, "3", "latte");
    tx.meta_mut().categories = vec!["coffee".into()];
    vault.add_transaction(tx).unwrap();
    vault
        .update_category("coffee", Some("Кофе"), CategoryGroup::Fixed, None, None)
        .unwrap();
    assert_eq!(vault.doc.transactions[0].meta().categories, vec!["coffee"]);

    assert!(vault.delete_category("coffee").is_err());
    vault.delete_category("coffee_2").unwrap();
    // an empty name returns to the built-in label
    let reset = vault
        .update_category("taxi", Some("  "), CategoryGroup::Daily, None, None)
        .unwrap();
    assert_eq!(reset.name, None);
}

#[test]
fn system_categories_cannot_be_changed() {
    let (_dir, mut vault) = setup();
    assert!(
        vault
            .update_category("trading", Some("Mine"), CategoryGroup::Service, None, None)
            .is_err()
    );
    assert!(vault.archive_category("fee", true).is_err());
    assert!(vault.delete_category("exchange").is_err());
    assert!(
        vault
            .create_category(None, "Sneaky", CategoryGroup::Service)
            .is_err()
    );
    // a user's category cannot be moved into the service group either
    assert!(
        vault
            .update_category("taxi", None, CategoryGroup::Service, None, None)
            .is_err()
    );
    assert!(
        vault
            .doc
            .categories
            .iter()
            .any(|category| category.key == "trading" && !category.archived)
    );
}

fn defaults() -> VaultDoc {
    UnlockedVault::in_memory(VaultDoc::default()).doc.clone()
}

#[test]
fn deciders_pick_the_key_and_refuse_system_categories_on_a_bare_document() {
    let doc = defaults();
    let coffee = plan_create_category(&doc, None, "Coffee", CategoryGroup::Daily).unwrap();
    assert_eq!(coffee.key, "coffee");
    let taken = plan_create_category(&doc, Some("taxi"), "Cab", CategoryGroup::Daily).unwrap();
    assert_eq!(taken.key, "taxi_2");
    assert!(plan_create_category(&doc, None, " ", CategoryGroup::Daily).is_err());
    assert!(plan_create_category(&doc, None, "X", CategoryGroup::Service).is_err());
    assert!(ensure_editable(&doc, "trading").is_err());
    assert!(ensure_editable(&doc, "taxi").is_ok());
    assert!(matches!(
        plan_update_category(&doc, "nope", None, CategoryGroup::Daily, None, None),
        Err(Error::Missing(_))
    ));
    let reset = plan_update_category(
        &doc,
        "taxi",
        Some(" "),
        CategoryGroup::Fixed,
        Some(true),
        None,
    )
    .unwrap();
    assert_eq!(
        (reset.name, reset.group, reset.essential),
        (None, CategoryGroup::Fixed, Some(true))
    );
    assert!(plan_archive_category(&doc, "fee", true).is_err());
    assert!(plan_archive_category(&doc, "taxi", true).unwrap().archived);
}

#[test]
fn deleting_needs_an_existing_unused_category() {
    let mut doc = defaults();
    assert!(matches!(
        plan_delete_category(&doc, "nope", &[]),
        Err(Error::Missing(_))
    ));
    assert!(plan_delete_category(&doc, "taxi", &[]).is_ok());
    doc.categorization_rules
        .push(new_rule(&doc, "r1".into(), "uber", "taxi"));
    assert!(matches!(
        plan_delete_category(&doc, "taxi", &[]),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn rules_are_checked_ordered_and_reordered_on_a_bare_document() {
    let mut doc = defaults();
    assert!(check_rule(&doc, " ", "taxi").is_err());
    assert!(check_rule(&doc, "uber", "nope").is_err());
    assert!(check_rule(&doc, "uber", "taxi").is_ok());
    for (id, pattern) in [("a", "one"), ("b", "two"), ("c", "three")] {
        let rule = new_rule(&doc, id.into(), pattern, "taxi");
        doc.categorization_rules.push(rule);
    }
    let orders: Vec<u32> = categorization_rules(&doc)
        .iter()
        .map(|rule| rule.order)
        .collect();
    assert_eq!(orders, [0, 1, 2]);
    let plan = plan_rule_order(&doc, &["c".to_string(), "a".to_string()]);
    assert_eq!(
        plan,
        [
            ("c".to_string(), 0),
            ("a".to_string(), 1),
            ("b".to_string(), 2)
        ]
    );
}

#[test]
fn the_rule_delegates_equal_the_scenarios_and_reordering_is_one_write() {
    let (_first_dir, mut delegated) = empty_vault();
    let (_second_dir, mut direct) = empty_vault();
    let first = delegated
        .create_categorization_rule("uber", "taxi")
        .unwrap();
    let second = scenarios::create_categorization_rule(&mut direct, "uber", "taxi").unwrap();
    assert_eq!((first.pattern, first.order), (second.pattern, second.order));
    let other = delegated
        .create_categorization_rule("metro", "taxi")
        .unwrap();
    delegated
        .update_categorization_rule(&other.id, " bus ", "taxi", false)
        .unwrap();
    assert_eq!(delegated.categorization_rules()[1].pattern, "bus");
    delegated
        .reorder_categorization_rules(std::slice::from_ref(&other.id))
        .unwrap();
    assert_eq!(delegated.categorization_rules()[0].id, other.id);
    delegated.delete_categorization_rule(&other.id).unwrap();
    assert!(delegated.delete_categorization_rule(&other.id).is_err());
    assert!(
        delegated
            .update_categorization_rule("nope", "x", "taxi", true)
            .is_err()
    );
    assert_eq!(delegated.categorization_rules().len(), 1);
}

#[test]
fn the_category_delegates_equal_the_scenarios() {
    let (_first_dir, mut delegated) = empty_vault();
    let (_second_dir, mut direct) = empty_vault();
    let made = delegated
        .create_category(None, "Coffee", CategoryGroup::Daily)
        .unwrap();
    let made_directly =
        scenarios::create_category(&mut direct, None, "Coffee", CategoryGroup::Daily).unwrap();
    assert_eq!(made, made_directly);
    delegated.archive_category("coffee", true).unwrap();
    scenarios::archive_category(&mut direct, "coffee", true).unwrap();
    assert_eq!(delegated.doc.categories, direct.doc.categories);
    assert!(
        delegated
            .doc
            .categories
            .iter()
            .any(|category| category.key == "coffee" && category.archived)
    );
    delegated.delete_category("coffee").unwrap();
    scenarios::delete_category(&mut direct, "coffee").unwrap();
    assert_eq!(delegated.doc.categories, direct.doc.categories);
    assert!(delegated.delete_category("coffee").is_err());
}
