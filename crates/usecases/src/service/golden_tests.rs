//! Characterization ("golden") tests: each pins an output that a later fix
//! must NOT change. All instants are fixed; nothing reads the wall clock.

use super::Session;
use super::test_support::*;
use crate::model::store::{Binding, StoreKind, SubType};
use mwt_reports::overview::{Report, ReportFilter, ReportKind};

/// BTC store (`Cold`): buy 1 @ 40 000 (Jan 1), buy 1 @ 50 000 (Jan 2),
/// sell 1 @ 60 000 (`sale_day`). Base currency usd.
fn btc_stream(sale_day: &str) -> (tempfile::TempDir, Session, String) {
    let (dir, mut vault) = setup();
    vault.set_base_currency("usd");
    let store = vault.doc.stores[1].id.clone();
    let mut first = income_of(&store, btc(), "1", "40000", "buy");
    at(&mut first, "2026-01-01T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, first).unwrap();
    let mut second = income_of(&store, btc(), "1", "50000", "buy");
    at(&mut second, "2026-01-02T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, second).unwrap();
    let mut sale = sell_btc(&store, "1", "60000", "60000");
    at(&mut sale, sale_day);
    crate::usecases::money::transactions::add_transaction(&mut vault, sale).unwrap();
    (dir, vault, store)
}

fn avg_cost_of(vault: &Session, store: &str) -> Option<crate::Amount> {
    vault
        .doc
        .holdings
        .iter()
        .find(|holding| holding.store_id == store && holding.asset.key() == btc().key())
        .and_then(|holding| holding.avg_cost)
}

/// G1: a sale does not move the average cost.
#[test]
fn golden_g1_holdings_after_buy_buy_sell() {
    let (_dir, vault, store) = btc_stream("2026-01-03T00:00:00Z");
    assert_eq!(holding_of(&vault, &store, &btc()), amount("1"));
    assert_eq!(avg_cost_of(&vault, &store), Some(amount("45000")));
}

/// G2: realized PnL of the same stream.
#[test]
fn golden_g2_realized_pnl() {
    let (_dir, vault, _store) = btc_stream("2026-01-03T00:00:00Z");
    let pnl = crate::money::portfolio::realized_pnl(&vault.books());
    assert_eq!(pnl.total_base, amount("15000"));
    assert_eq!(pnl.legs.len(), 1);
    assert_eq!(pnl.legs[0].side, "sell");
    assert_eq!(pnl.fees_base, amount("0"));
}

/// G3: unrealized PnL at a BTC quote of 60 000.
#[test]
fn golden_g3_unrealized_pnl() {
    let (_dir, mut vault, store) = btc_stream("2026-01-03T00:00:00Z");
    crate::usecases::money::rates::apply_prices(
        &mut vault,
        &[crate::model::vault::PricePoint {
            asset_key: btc().key(),
            value_usd: amount("60000"),
            ts: "2026-01-03T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        }],
    );
    let overview = crate::money::portfolio::store_overview(&vault.books(), &store, true).unwrap();
    assert_eq!(overview.unrealized_base, amount("15000"));
}

/// G4: the PnL report puts the February sale into February.
#[test]
fn golden_g4_pnl_report_by_month() {
    let (_dir, vault, _store) = btc_stream("2026-02-03T00:00:00Z");
    let filter = ReportFilter {
        from: Some("2026-01".into()),
        to: Some("2026-02".into()),
        ..ReportFilter::default()
    };
    let Report::Pnl(report) = crate::reports::overview::report(
        &vault.books(),
        ReportKind::Pnl,
        &filter,
        secs("2026-03-01T12:00:00Z"),
        "UTC",
    )
    .unwrap() else {
        panic!("a pnl report")
    };
    assert_eq!(report.rows.len(), 2);
    assert_eq!(report.rows[0].realized, amount("0"));
    assert_eq!(report.rows[1].realized, amount("15000"));
    assert_eq!(report.total_realized, amount("15000"));
    assert!(report.has_data);
}

/// G5: merging two BTC token accounts keeps quantity, average and PnL.
#[test]
fn golden_g5_merge_without_corrections() {
    use crate::money::store_change::{StoreChange, StoreChangeInput};
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    let wallet = store(&mut vault, "Wallet", StoreKind::Crypto, None);
    let token = |vault: &mut Session, name: &str| {
        crate::usecases::money::stores::create_store_with(
            vault,
            name,
            StoreKind::Crypto,
            "#000",
            "",
            Some(wallet.clone()),
            super::StoreExtras {
                sub_type: Some(SubType::Token),
                binding: Some(Binding::Asset(btc())),
                ..super::StoreExtras::default()
            },
        )
        .unwrap()
        .id
    };
    let first = token(&mut vault, "BTC A");
    let second = token(&mut vault, "BTC B");
    let mut buy_a = income_of(&first, btc(), "1", "40000", "buy");
    at(&mut buy_a, "2026-01-01T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, buy_a).unwrap();
    let mut buy_b = income_of(&second, btc(), "1", "50000", "buy");
    at(&mut buy_b, "2026-01-02T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, buy_b).unwrap();
    let realized_before = crate::money::portfolio::realized_pnl(&vault.books()).total_base;

    let plan = crate::usecases::money::store_change::apply_store_change(
        &mut vault,
        &StoreChangeInput {
            store_id: first.clone(),
            change: StoreChange::MergeInto {
                target_id: second.clone(),
            },
        },
    )
    .unwrap();

    assert_eq!(plan.moved_transactions, 1);
    assert!(crate::money::tree::find_store(&vault.doc, &first).is_none());
    assert_eq!(holding_of(&vault, &second, &btc()), amount("2"));
    assert_eq!(avg_cost_of(&vault, &second), Some(amount("45000")));
    assert_eq!(
        crate::money::portfolio::realized_pnl(&vault.books()).total_base,
        realized_before
    );
}
