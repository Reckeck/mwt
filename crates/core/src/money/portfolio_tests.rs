//! Pins of the portfolio views: the holdings fold and the queries over `Books`
//! agree with the delegates they replace, and neither writes.

use super::portfolio::*;
use crate::model::store::StoreKind;
use crate::service::test_support::*;

#[test]
fn compute_holdings_matches_the_stored_holdings_and_writes_nothing() {
    let (_dir, mut vault) = empty_vault();
    let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
    vault
        .add_transaction(income_of(&cold, btc(), "2", "50000", "buy"))
        .unwrap();
    let before = vault.doc.clone();
    let folded = compute_holdings(&vault.books());
    assert_eq!(vault.doc, before, "the fold is a query");
    assert_eq!(folded.len(), vault.doc.holdings.len());
    let held = folded
        .iter()
        .find(|holding| holding.store_id == cold && holding.asset.key() == btc().key())
        .unwrap();
    assert_eq!(held.quantity, amount("2"));
    assert_eq!(held.avg_cost, Some(amount("50000")));
}

#[test]
fn queries_over_books_equal_the_delegates() {
    let (_dir, mut vault) = empty_vault();
    let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
    vault
        .add_transaction(income_of(&cold, btc(), "1", "50000", "buy"))
        .unwrap();
    let books = vault.books();
    assert_eq!(store_tree(&books), vault.store_tree());
    assert_eq!(portfolio_summary(&books), vault.portfolio_summary());
    assert_eq!(realized_pnl(&books), vault.realized_pnl());
    assert_eq!(
        store_overview(&books, &cold, true).unwrap(),
        vault.store_overview(&cold, true).unwrap()
    );
    assert_eq!(store_pnl(&books, &cold, true), vault.store_pnl(&cold, true));
}
