//! Tests of the cost_basis area that need the vault (`Session`): moved from
//! `money/cost_basis_tests.rs` with their assertions unchanged (crate-split T13).

use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::service::test_support::*;

fn at(tx: &mut Transaction, ts: &str) {
    tx.meta_mut().timestamp = ts.into();
}

fn move_priced(from: &str, to: &str, quantity: &str, price: &str) -> Transaction {
    let mut tx = transfer(from, to, quantity);
    if let Transaction::Transfer(inner) = &mut tx {
        inner.price = usd_price(price);
    }
    tx
}

/// The review's example: buy 1 BTC at 20 000, move at 60 000, sell at 60 000.
#[test]
fn move_carries_the_source_cost_and_the_later_sale_realizes_the_gain() {
    let (_dir, mut vault) = empty_vault();
    let hot = store(&mut vault, "Hot", StoreKind::Crypto, None);
    let cold = store(&mut vault, "Cold", StoreKind::Exchange, None);
    let mut buy = income_of(&hot, btc(), "1", "20000", "buy");
    at(&mut buy, "2026-01-01T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, buy).unwrap();
    let mut moved = move_priced(&hot, &cold, "1", "60000");
    at(&mut moved, "2026-01-02T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, moved).unwrap();

    let held = |vault: &crate::service::Session| {
        let holding = vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == cold && holding.asset.key() == btc().key())
            .unwrap();
        (holding.quantity, holding.avg_cost)
    };
    assert_eq!(held(&vault), (amount("1"), Some(amount("20000"))));
    assert!(
        crate::money::portfolio::realized_pnl(&vault.books())
            .legs
            .is_empty(),
        "a move realizes nothing"
    );

    let mut sell = sell_btc(&cold, "1", "60000", "60000");
    at(&mut sell, "2026-01-03T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, sell).unwrap();
    let pnl = crate::money::portfolio::realized_pnl(&vault.books());
    assert_eq!(pnl.total_base, amount("40000"));
    assert_eq!(pnl.legs.len(), 1);
    // The per-store view of the selling wallet sees the carried cost too.
    assert_eq!(
        crate::money::portfolio::store_pnl(&vault.books(), &cold, false).total_base,
        amount("40000")
    );
}
