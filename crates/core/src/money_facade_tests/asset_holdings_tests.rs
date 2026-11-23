//! Pins of the asset holdings view: one asset across every wallet.

use crate::model::chain::Chain;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::{PriceStamp, Transaction};
use crate::service::test_support::*;

fn usdt_on(chain: Chain, contract: &str) -> AssetRef {
    AssetRef {
        contract: Some(contract.into()),
        ..AssetRef::crypto("USDT", "Tether", None, chain)
    }
}

#[test]
fn asset_holdings_join_the_same_ticker_across_wallets() {
    let (_dir, mut vault) = empty_vault();
    let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
    let hot = store(&mut vault, "Hot", StoreKind::Crypto, None);
    let tether = AssetRef::crypto("USDT", "Tether", Some("tether"), Chain::Ethereum);
    let bare = AssetRef::crypto("USDT", "Tether", None, Chain::Ethereum);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&cold, tether.clone(), "100", "1", "in"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&hot, bare, "50", "1", "in"),
    )
    .unwrap();
    let held = crate::money::asset_holdings::asset_holdings(&vault.books(), &tether);
    assert_eq!(held.rows.len(), 2);
    assert_eq!(held.total_quantity, amount("150"));
    assert_eq!(held.rows[0].store_name, "Cold");
}

#[test]
fn asset_holdings_keep_other_chains_apart() {
    let (_dir, mut vault) = empty_vault();
    let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
    let erc = usdt_on(
        Chain::Ethereum,
        "0xdac17f958d2ee523a2206206994597c13d831ec7",
    );
    let trc = usdt_on(Chain::Tron, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t");
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&cold, erc.clone(), "10", "1", "in"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&cold, trc, "20", "1", "in"),
    )
    .unwrap();
    let held = crate::money::asset_holdings::asset_holdings(&vault.books(), &erc);
    assert_eq!(held.rows.len(), 1);
    assert_eq!(held.total_quantity, amount("10"));
}

#[test]
fn asset_holdings_leave_derivatives_out() {
    assert!(!btc().same_as(&AssetRef::perp("bybit", "BTC")));
    assert!(!AssetRef::perp("bybit", "BTC").same_as(&btc()));
    assert!(btc().same_as(&btc()));
}

#[test]
fn asset_holdings_flag_peg_valued_stablecoins() {
    let (_dir, mut vault) = empty_vault();
    let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
    let tether = AssetRef::crypto("USDT", "Tether", Some("tether"), Chain::Ethereum);
    let mut topup = income_of(&cold, tether.clone(), "5", "1", "in");
    if let Transaction::Topup(inner) = &mut topup {
        inner.price = PriceStamp::default();
    }
    crate::usecases::money::transactions::add_transaction(&mut vault, topup).unwrap();
    let held = crate::money::asset_holdings::asset_holdings(&vault.books(), &tether);
    assert!(held.rows[0].peg_valued);
    assert_eq!(held.rows[0].value_base, amount("5"));
}
