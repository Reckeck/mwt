//! Pins of `CostStrategy`: a move carries the source's average cost, a
//! Convert and an AssetSell realize on the outgoing leg, and the balance
//! replay and the PnL replay agree on it.

use super::*;
use crate::model::store::AssetRef;
use crate::model::store::StoreKind;
use crate::model::transaction::{
    AccountRef, AssetSellTx, ConvertSide, ConvertTx, PriceStamp, Transaction,
};
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
    vault.add_transaction(buy).unwrap();
    let mut moved = move_priced(&hot, &cold, "1", "60000");
    at(&mut moved, "2026-01-02T00:00:00Z");
    vault.add_transaction(moved).unwrap();

    let held = |vault: &crate::service::UnlockedVault| {
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
        vault.realized_pnl().legs.is_empty(),
        "a move realizes nothing"
    );

    let mut sell = sell_btc(&cold, "1", "60000", "60000");
    at(&mut sell, "2026-01-03T00:00:00Z");
    vault.add_transaction(sell).unwrap();
    let pnl = vault.realized_pnl();
    assert_eq!(pnl.total_base, amount("40000"));
    assert_eq!(pnl.legs.len(), 1);
    // The per-store view of the selling wallet sees the carried cost too.
    assert_eq!(vault.store_pnl(&cold, false).total_base, amount("40000"));
}

#[test]
fn strategy_carries_unknown_cost_as_unknown() {
    let mut strategy = CostStrategy::default();
    let mut moved = transfer("a", "b", "1").legs();
    for leg in &mut moved {
        leg.amount = if leg.store_id == "a" {
            -amount("1")
        } else {
            amount("1")
        };
    }
    strategy.apply_transaction(moved, |_| amount("60000"));
    let key = PositionKey {
        store_id: "b".into(),
        asset_key: btc().key(),
    };
    assert_eq!(strategy.position(&key).quantity, amount("1"));
    assert_eq!(strategy.position(&key).cost, None);
}

/// A Convert between own wallets is a sale of the outgoing asset, not a move.
#[test]
fn convert_realizes_on_the_outgoing_leg() {
    let mut strategy = CostStrategy::default();
    let buy = income_of("a", btc(), "1", "20000", "buy").legs();
    strategy.apply_transaction(buy, |_| amount("20000"));
    let side = |store: &str, asset: AssetRef, raw: &str| ConvertSide {
        account: AccountRef::Store(store.into()),
        asset,
        amount: amount(raw),
        price: PriceStamp::default(),
    };
    let convert = Transaction::Convert(ConvertTx {
        meta: income_of("a", btc(), "1", "0", "convert").meta().clone(),
        from: side("a", btc(), "1"),
        to: side("b", usd(), "60000"),
        fee: None,
        rate_override: None,
    });
    let replayed = strategy.apply_transaction(convert.legs(), |_| amount("60000"));
    let realized = replayed[0].realized.unwrap();
    assert_eq!(realized.quantity, amount("1"));
    assert_eq!(realized.pnl_base, amount("40000"));
}

/// An AssetSell with proceeds into a tracked wallet still realizes.
#[test]
fn asset_sell_realizes_with_tracked_proceeds() {
    let mut strategy = CostStrategy::default();
    let buy = income_of("flat", btc(), "1", "20000", "buy").legs();
    strategy.apply_transaction(buy, |_| amount("20000"));
    let sell = Transaction::AssetSell(AssetSellTx {
        meta: income_of("flat", btc(), "1", "0", "sell").meta().clone(),
        store_id: "flat".into(),
        asset: btc(),
        unit_price: amount("60000"),
        quantity: amount("1"),
        currency: usd(),
        proceeds_to: Some("bank".into()),
        fee: None,
        price: PriceStamp::default(),
    });
    let replayed = strategy.apply_transaction(sell.legs(), |_| amount("60000"));
    assert_eq!(replayed[0].realized.unwrap().pnl_base, amount("40000"));
}

/// A buy that pays its fee in the bought asset keeps the replay-order average.
#[test]
fn buy_with_fee_in_the_bought_asset_averages_in_leg_order() {
    let mut strategy = CostStrategy::default();
    let first = income_of("a", btc(), "1", "20000", "first").legs();
    strategy.apply_transaction(first, |_| amount("20000"));
    let mut second = income_of("a", btc(), "1", "30000", "second").legs();
    let mut fee = second[0].clone();
    fee.amount = -amount("0.01");
    fee.role = LegRole::Fee;
    second.push(fee);
    strategy.apply_transaction(second, |_| amount("30000"));
    let key = PositionKey {
        store_id: "a".into(),
        asset_key: btc().key(),
    };
    let position = strategy.position(&key);
    assert_eq!(position.quantity, amount("1.99"));
    assert_eq!(position.cost, Some(amount("25000")));
}
