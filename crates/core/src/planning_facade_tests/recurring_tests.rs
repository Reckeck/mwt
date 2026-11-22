//! Tests of the recurring-payment check: the old inline tests (names and bodies
//! unchanged), then the pure function on a bare document.

use crate::model::transaction::Transaction;
use crate::planning::recurring::*;
use crate::service::test_support::{amount, at, empty_vault, payment};

fn subscription(raw: &str, stamp: &str) -> Transaction {
    let mut tx = payment("s1", raw, "Music+");
    tx.meta_mut().categories = vec!["subscriptions".into()];
    at(&mut tx, stamp);
    tx
}

#[test]
fn first_payment_is_new() {
    let (_dir, vault) = empty_vault();
    let check = crate::planning::recurring::recurring_check(
        &vault.doc,
        &subscription("5", "2026-05-10T10:00:00Z"),
    );
    assert!(check.new);
    assert!(!check.grew);
    assert_eq!(check.previous, None);
}

#[test]
fn dearer_payment_is_seen() {
    let (_dir, mut vault) = empty_vault();
    vault
        .doc
        .transactions
        .push(subscription("5", "2026-04-10T10:00:00Z"));
    let check = crate::planning::recurring::recurring_check(
        &vault.doc,
        &subscription("6", "2026-05-10T10:00:00Z"),
    );
    assert!(!check.new);
    assert!(check.grew);
    assert_eq!(check.previous, Some(amount("5")));
    assert_eq!(check.change_pct, Some(amount("20")));
}

#[test]
fn same_amount_did_not_grow() {
    let (_dir, mut vault) = empty_vault();
    vault
        .doc
        .transactions
        .push(subscription("5", "2026-04-10T10:00:00Z"));
    let check = crate::planning::recurring::recurring_check(
        &vault.doc,
        &subscription("5", "2026-05-10T10:00:00Z"),
    );
    assert!(!check.new);
    assert!(!check.grew);
}

#[test]
fn old_or_other_payee_does_not_count() {
    let (_dir, mut vault) = empty_vault();
    vault
        .doc
        .transactions
        .push(subscription("5", "2026-02-01T10:00:00Z"));
    let mut other = subscription("5", "2026-05-01T10:00:00Z");
    other.meta_mut().comment = "Cinema".into();
    vault.doc.transactions.push(other);
    let check = crate::planning::recurring::recurring_check(
        &vault.doc,
        &subscription("6", "2026-05-10T10:00:00Z"),
    );
    assert!(check.new);
}

#[test]
fn a_bare_document_gives_the_same_answer_as_the_vault() {
    let (_dir, mut vault) = empty_vault();
    vault
        .doc
        .transactions
        .push(subscription("5", "2026-04-10T10:00:00Z"));
    let next = subscription("6", "2026-05-10T10:00:00Z");
    assert_eq!(
        recurring_check(&vault.doc, &next),
        crate::planning::recurring::recurring_check(&vault.doc, &next)
    );
    assert_eq!(
        recurring_check(&crate::model::VaultDoc::default(), &next),
        RecurringCheck {
            new: true,
            grew: false,
            previous: None,
            change_pct: None,
        }
    );
}
