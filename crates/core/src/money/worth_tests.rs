//! Tests of the worth area: the old inline tests (names kept), which drive
//! the same rules through the `UnlockedVault` delegates.

use super::worth::*;
use crate::Amount;
use crate::model::planning::Goal;
use crate::model::store::{AssetRef, LiabilityKind, LiabilityTerms, StoreScope};
use crate::model::store::{Holding, Store, StoreKind};
use crate::model::transaction::{AccountRef, Transaction};
use crate::model::vault::VaultDoc;
use crate::service::StoreExtras;
use crate::service::UnlockedVault;
use crate::service::test_support::*;

/// A bank and a credit card: draw 500, pay 200 at a shop, 10 interest,
/// repay 300 (the repayment is priced at 1, as an app-booked one is).
fn credit_card_story(vault: &mut UnlockedVault) -> (String, Transaction, Transaction) {
    vault.set_base_currency("usd");
    quote_usd(vault);
    let bank = store(vault, "Bank", StoreKind::Bank, None);
    let card = vault
        .create_store_with(
            "Card",
            StoreKind::Liability,
            "",
            "",
            None,
            StoreExtras {
                liability: Some(LiabilityTerms {
                    kind: LiabilityKind::CreditCard,
                    rate: None,
                    limit: Some(amount("1000")),
                    due_day: Some(25),
                    monthly_payment: None,
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    let loan = transfer_between(
        AccountRef::Store(card.clone()),
        AccountRef::Store(bank.clone()),
        usd(),
        "500",
    );
    vault.add_transaction(loan).unwrap();
    vault
        .add_transaction(payment(&card, "200", "shop"))
        .unwrap();
    let mut interest = payment(&card, "10", "interest");
    interest.meta_mut().categories = vec!["loan_interest".into()];
    vault.add_transaction(interest.clone()).unwrap();
    let mut repay = transfer_between(
        AccountRef::Store(bank),
        AccountRef::Store(card.clone()),
        usd(),
        "300",
    );
    if let Transaction::Transfer(transfer) = &mut repay {
        transfer.price = usd_price("1");
    }
    vault.add_transaction(repay.clone()).unwrap();
    (card, interest, repay)
}

/// A credit card: draw the loan, buy with the card, interest, pay back.
#[test]
fn a_loan_is_a_negative_balance_and_a_liability() {
    let (_dir, mut vault) = empty_vault();
    let (card, interest, repay) = credit_card_story(&mut vault);

    assert_eq!(holding_of(&vault, &card, &usd()), amount("-410"));
    let worth = vault.net_worth();
    assert_eq!(worth.financial, amount("200"));
    assert_eq!(worth.liabilities, amount("410"));
    assert_eq!(
        worth.liabilities_by_kind,
        vec![LiabilityShare {
            kind: LiabilityKind::CreditCard,
            amount: amount("410"),
        }]
    );
    assert_eq!(worth.net, amount("-210"));
    let summary = vault.portfolio_summary();
    assert_eq!(summary.liabilities_base, amount("410"));
    assert_eq!(summary.net_worth_base, amount("-210"));

    // paying the loan off is a mandatory cost; the card is outside the budget
    assert_eq!(vault.classify_flow(&repay), FlowClass::Fixed);
    assert_eq!(vault.classify_flow(&interest), FlowClass::Ignored);
}

/// A fiat card is never a position: no profit, no cost, no unrealized
/// result, however the repayment is priced.
#[test]
fn a_liability_store_books_no_profit() {
    use crate::service::TxFilter;
    use crate::service::reports::{Report, ReportFilter, ReportKind};
    let (_dir, mut vault) = empty_vault();
    let (card, _interest, _repay) = credit_card_story(&mut vault);

    let pnl = vault.realized_pnl();
    assert_eq!(pnl.total_base, Amount::ZERO);
    assert!(pnl.legs.is_empty());
    let holding = vault
        .doc
        .holdings
        .iter()
        .find(|holding| holding.store_id == card)
        .unwrap();
    assert_eq!(holding.quantity, amount("-410"));
    assert_eq!(holding.avg_cost, None);
    assert_eq!(
        vault.store_overview(&card, true).unwrap().unrealized_base,
        Amount::ZERO
    );
    let Report::Pnl(report) = vault
        .report(
            ReportKind::Pnl,
            &ReportFilter::default(),
            secs("2026-03-01T12:00:00Z"),
            "UTC",
        )
        .unwrap()
    else {
        panic!("a pnl report")
    };
    assert!(!report.has_data);
    assert_eq!(report.total_realized, Amount::ZERO);
    let filter = TxFilter {
        realized: true,
        ..TxFilter::default()
    };
    assert!(vault.list_transactions(&filter, "UTC").unwrap().is_empty());
    let worth = vault.net_worth();
    assert_eq!(worth.liabilities, amount("410"));
    assert_eq!(worth.net, amount("-210"));
}

/// Plan §7.3, one row at a time.
#[test]
fn flows_are_classified_for_the_budget() {
    let (_dir, mut vault) = empty_vault();
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    let usd_account = store(&mut vault, "USD", StoreKind::Bank, Some(&bank));
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let exchange = store(&mut vault, "Bybit", StoreKind::Exchange, None);
    let piggy = vault
        .create_store_with(
            "Piggy",
            StoreKind::Bank,
            "",
            "",
            None,
            StoreExtras {
                purpose: Some(crate::model::store::Purpose::Investment),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    vault.doc.goals.push(Goal {
        id: "goal".into(),
        name: "Car".into(),
        active: true,
        target_amount: amount("1000"),
        currency: "usd".into(),
        starts_at: "2026-01-01T00:00:00Z".into(),
        deadline: None,
        priority: 3,
        monthly_commitment: None,
        scope: StoreScope::store(&piggy, true),
        created_at: "2026-01-01T00:00:00Z".into(),
        scenario_id: None,
        expected_return: None,
    });
    let store_ref = |id: &str| AccountRef::Store(id.to_string());
    let with_category = |mut tx: Transaction, key: &str| {
        tx.meta_mut().categories = vec![key.to_string()];
        tx
    };
    let cases = [
        (income(&bank, "1", "salary"), FlowClass::Income),
        (
            with_category(income(&bank, "1", "x"), "trading"),
            FlowClass::Ignored,
        ),
        (
            with_category(payment(&bank, "1", "rent"), "rent_mortgage"),
            FlowClass::Fixed,
        ),
        (payment(&cash, "1", "food"), FlowClass::Daily),
        (
            with_category(payment(&bank, "1", "save"), "savings"),
            FlowClass::Savings,
        ),
        (
            payment(&exchange, "1", "outside the budget"),
            FlowClass::Ignored,
        ),
        (
            transfer_between(store_ref(&bank), store_ref(&usd_account), usd(), "1"),
            FlowClass::Ignored,
        ),
        (
            transfer_between(store_ref(&bank), store_ref(&cash), usd(), "1"),
            FlowClass::Ignored,
        ),
        (
            transfer_between(store_ref(&bank), store_ref(&piggy), usd(), "1"),
            FlowClass::Savings,
        ),
        (
            transfer_between(store_ref(&exchange), store_ref(&bank), usd(), "1"),
            FlowClass::BudgetIn,
        ),
        (
            transfer_between(store_ref(&bank), store_ref(&exchange), usd(), "1"),
            FlowClass::BudgetOut,
        ),
        (
            transfer_between(
                store_ref(&cash),
                AccountRef::Raw("market".into()),
                usd(),
                "1",
            ),
            FlowClass::Daily,
        ),
        (
            with_category(
                transfer_between(
                    AccountRef::Raw("client".into()),
                    store_ref(&bank),
                    usd(),
                    "1",
                ),
                "freelance",
            ),
            FlowClass::Income,
        ),
        (
            transfer_between(
                AccountRef::Raw("refund".into()),
                store_ref(&bank),
                usd(),
                "1",
            ),
            FlowClass::Ignored,
        ),
    ];
    for (index, (tx, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            vault.classify_flow(&tx),
            expected,
            "case {index}: {}",
            tx.meta().comment
        );
    }
}

/// Futures notional is not the user's money.
#[test]
fn derivatives_never_count_in_value() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    vault.doc.price_cache.push(crate::model::vault::PricePoint {
        asset_key: AssetRef::perp("bybit", "BTCUSDT").key(),
        value_usd: amount("60000"),
        ts: "2000-01-01T00:00:00Z".into(),
        source: "test".into(),
        change_24h_pct: None,
    });
    let open = Transaction::TradeFutures(crate::model::transaction::TradeFuturesTx {
        meta: meta("perp", &[]),
        store_id: store.clone(),
        contract: AssetRef::perp("bybit", "BTCUSDT"),
        settle: AssetRef::fiat("USDT", "Tether"),
        direction: crate::model::transaction::FuturesDirection::Long,
        action: crate::model::transaction::FuturesAction::Open,
        quantity: amount("2"),
        fill_price: amount("60000"),
        leverage: None,
        fee: None,
        closed_pnl: None,
        order_id: None,
        category: crate::model::transaction::FuturesCategory::Linear,
        price: crate::model::transaction::PriceStamp::default(),
    });
    vault.add_transaction(open).unwrap();
    assert_eq!(vault.portfolio_summary().total_usd, Amount::ZERO);
    assert_eq!(vault.net_worth().net, Amount::ZERO);
    let overview = vault.store_overview(&store, true).unwrap();
    assert!(overview.positions.is_empty());
    assert_eq!(overview.open_positions.len(), 1);
    assert!(
        vault
            .history_series("usd")
            .iter()
            .all(|point| point.value_usd.is_zero())
    );
}

/// A fee is a cost, not a sale: buying and selling at one price with fees
/// realizes nothing, and the fees are reported on their own.
#[test]
fn fees_do_not_fake_a_profit() {
    let (_dir, mut vault) = setup();
    vault.set_base_currency("usd");
    let store = vault.doc.stores[1].id.clone();
    let mut deposit = income(&store, "1000", "in");
    at(&mut deposit, "2026-01-01T00:00:00Z");
    vault.add_transaction(deposit).unwrap();
    let Transaction::TradeSpot(mut buy) = trade(&store, "500", "0.01", "buy") else {
        unreachable!()
    };
    buy.sold.price = usd_price("1");
    buy.bought.price = usd_price("50000");
    buy.fee = fee_usd("2");
    let mut buy = Transaction::TradeSpot(buy);
    at(&mut buy, "2026-01-02T00:00:00Z");
    vault.add_transaction(buy).unwrap();
    let Transaction::TradeSpot(mut sell) = sell_btc(&store, "0.01", "500", "50000") else {
        unreachable!()
    };
    sell.fee = fee_usd("2");
    let mut sell = Transaction::TradeSpot(sell);
    at(&mut sell, "2026-01-03T00:00:00Z");
    vault.add_transaction(sell).unwrap();

    let pnl = vault.realized_pnl();
    assert_eq!(pnl.total_base, Amount::ZERO);
    assert_eq!(pnl.fees_base, amount("4"));
    assert_eq!(holding_of(&vault, &store, &usd()), amount("996"));
}

/// A bare document: a bank (in the budget), a property wallet holding `FLAT`.
fn bare_doc() -> (VaultDoc, AssetRef) {
    let thing = AssetRef {
        symbol: "FLAT".into(),
        name: "Flat".into(),
        coingecko_id: None,
        chain: None,
        contract: None,
        quote: "FLAT".into(),
    };
    let mut doc = VaultDoc::default();
    let mut bank = Store::new("Bank", StoreKind::Bank, "");
    bank.id = "bank".into();
    let mut flat = Store::new("Flat", StoreKind::Property, "");
    flat.id = "flat".into();
    doc.stores.extend([bank, flat]);
    doc.holdings.push(Holding {
        id: "h1".into(),
        store_id: "flat".into(),
        asset: thing.clone(),
        quantity: amount("1"),
        avg_cost: None,
    });
    (doc, thing)
}

#[test]
fn plan_manual_price_refuses_bad_input_and_accepts_held_property() {
    let (doc, thing) = bare_doc();
    let key = thing.key();
    let plan = |price: &str, currency: &str, key: &str| {
        plan_manual_price(&doc, key, amount(price), currency, "now".into())
    };
    assert!(plan("0", "usd", &key).is_err());
    assert!(plan("-5", "usd", &key).is_err());
    assert!(plan("10", "  ", &key).is_err());
    assert!(plan("10", "usd", "asset:OTHER").is_err());
    let price = plan("120000", " USD ", &key).unwrap();
    assert_eq!(price.asset_key, key);
    assert_eq!(price.unit_price, amount("120000"));
    assert_eq!(price.currency, "usd");
    assert_eq!(price.updated_at, "now");
}

#[test]
fn classify_flow_reads_a_bare_document() {
    let (doc, _) = bare_doc();
    assert_eq!(
        classify_flow(&doc, &payment("bank", "5", "lunch")),
        FlowClass::Daily
    );
    assert_eq!(
        classify_flow(&doc, &payment("flat", "5", "x")),
        FlowClass::Ignored
    );
    assert!(goal_store_ids(&doc).is_empty());
}

#[test]
fn set_manual_price_through_the_delegate_equals_the_usecase() {
    let build = || {
        let (_dir, mut vault) = empty_vault();
        let flat = store(&mut vault, "Flat", StoreKind::Property, None);
        let (_, thing) = bare_doc();
        vault
            .transact(|tx| {
                tx.doc_mut().holdings.push(Holding {
                    id: "h1".into(),
                    store_id: flat.clone(),
                    asset: thing.clone(),
                    quantity: amount("1"),
                    avg_cost: None,
                });
                Ok(())
            })
            .unwrap();
        (_dir, vault, thing)
    };
    let (_dir_a, mut via_delegate, thing) = build();
    let (_dir_b, mut via_usecase, _) = build();
    let first = via_delegate
        .set_manual_price(&thing.key(), amount("7"), "usd")
        .unwrap();
    let second = crate::usecases::money::capital::set_manual_price(
        &mut via_usecase,
        &thing.key(),
        amount("7"),
        "usd",
    )
    .unwrap();
    assert_eq!(first.unit_price, second.unit_price);
    assert_eq!(first.currency, second.currency);
    assert_eq!(via_delegate.doc.manual_prices.len(), 1);
    assert_eq!(via_usecase.doc.manual_prices.len(), 1);
    assert_eq!(via_delegate.net_worth(), via_usecase.net_worth());
}
