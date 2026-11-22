//! Tests of the preview area: the before/after of a draft, nothing written.

use super::preview::*;
use crate::model::transaction::{FuturesAction, FuturesDirection, Transaction};
use crate::service::test_support::amount;

#[test]
fn estimates_liquidation_of_an_opened_position() {
    let mut tx = crate::model::fixtures::every_transaction()
        .into_iter()
        .find(|tx| matches!(tx, Transaction::TradeFutures(_)))
        .unwrap();
    let Transaction::TradeFutures(trade) = &mut tx else {
        unreachable!()
    };
    trade.action = FuturesAction::Open;
    let Transaction::TradeFutures(trade) = &mut tx else {
        unreachable!()
    };
    trade.fill_price = amount("89552");
    trade.leverage = Some(amount("5"));
    trade.direction = FuturesDirection::Long;
    assert_eq!(liquidation_price(&tx), Some(amount("72089.36")));
    let Transaction::TradeFutures(trade) = &mut tx else {
        unreachable!()
    };
    trade.direction = FuturesDirection::Short;
    assert_eq!(liquidation_price(&tx), Some(amount("107014.64")));
    let Transaction::TradeFutures(trade) = &mut tx else {
        unreachable!()
    };
    trade.leverage = None;
    assert_eq!(liquidation_price(&tx), None);
}
