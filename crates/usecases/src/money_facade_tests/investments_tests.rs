//! Pins of the investments view: coins joined by asset, cash left out.

use crate::model::chain::Chain;
use crate::model::store::{AssetRef, StoreKind};
use crate::service::test_support::*;

#[test]
fn investments_join_coins_and_leave_cash_out() {
    let (_dir, mut vault) = empty_vault();
    let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
    let hot = store(&mut vault, "Hot", StoreKind::Crypto, None);
    let btc = AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin);
    let usdt = AssetRef::crypto("USDT", "Tether", Some("tether"), Chain::Ethereum);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&cold, btc.clone(), "0.05", "60000", "buy"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&hot, btc, "0.05", "60000", "buy"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&hot, usdt, "500", "1", "cash"),
    )
    .unwrap();
    let report = crate::money::investments::investments(&vault.books(), &[]);
    assert_eq!(report.rows.len(), 1, "USDT is cash, not an investment");
    assert_eq!(report.rows[0].label, "BTC");
    assert_eq!(report.rows[0].quantity, amount("0.1"));
    assert_eq!(report.rows[0].invested, amount("6000"));
    // Only the hot wallet: half of it.
    assert_eq!(
        crate::money::investments::investments(&vault.books(), &[hot]).rows[0].quantity,
        amount("0.05")
    );
}
