//! Tests of the insights area: the old inline tests (names and bodies unchanged).

use crate::Amount;
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::time::unix_secs;
use mwt_reports::insights::*;
use mwt_reports::overview::{Report, ReportFilter, ReportKind};

const NOW: &str = "2026-06-15T12:00:00Z";

fn dated(mut tx: Transaction, ts: &str) -> Transaction {
    at(&mut tx, ts);
    tx
}

fn spend(store: &str, sum: &str, category: &str, ts: &str) -> Transaction {
    let mut tx = payment(store, sum, "spend");
    tx.meta_mut().categories = vec![category.into()];
    dated(tx, ts)
}

fn earn(store: &str, sum: &str, ts: &str) -> Transaction {
    dated(income(store, sum, "salary"), ts)
}

fn vault_with_card() -> (tempfile::TempDir, UnlockedVault, String) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    (dir, vault, card)
}

fn overview(vault: &UnlockedVault) -> ReportOverview {
    crate::reports::insights::report_overview(
        &vault.books(),
        &ReportFilter::default(),
        unix_secs(NOW).unwrap(),
        "UTC",
    )
    .unwrap()
}

fn codes(facts: &[Insight]) -> Vec<InsightCode> {
    facts.iter().map(|fact| fact.code).collect()
}

#[test]
fn overspending_names_the_gap_and_the_share_of_income() {
    let fact = balance_insight(amount("1000"), amount("1120")).unwrap();
    assert_eq!(fact.code, InsightCode::Overspent);
    assert_eq!(fact.amount, Some(amount("120")));
    assert_eq!(fact.pct, Some(amount("12")));
    let kept = balance_insight(amount("1000"), amount("750")).unwrap();
    assert_eq!(kept.code, InsightCode::Saved);
    assert_eq!(kept.pct, Some(amount("25")));
    assert_eq!(
        balance_insight(Amount::ZERO, amount("10")).unwrap().code,
        InsightCode::NoIncome
    );
    assert!(balance_insight(Amount::ZERO, Amount::ZERO).is_none());
}

#[test]
fn wealth_within_a_percent_reads_as_flat() {
    assert_eq!(
        wealth_change(amount("1000"), amount("1005")).code,
        InsightCode::WealthFlat
    );
    let down = wealth_change(amount("1000"), amount("800"));
    assert_eq!(down.code, InsightCode::WealthDown);
    assert_eq!(down.amount, Some(amount("200")));
    assert_eq!(down.pct, Some(amount("20")));
    assert_eq!(
        wealth_change(Amount::ZERO, amount("50")).code,
        InsightCode::WealthUp
    );
}

#[test]
fn every_report_carries_its_insights_in_the_meta() {
    let (_dir, mut vault, card) = vault_with_card();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&card, "1000", "2026-05-05T10:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "1200", "groceries", "2026-05-06T10:00:00Z"),
    )
    .unwrap();
    let report = crate::reports::overview::report(
        &vault.books(),
        ReportKind::Cashflow,
        &ReportFilter::default(),
        unix_secs(NOW).unwrap(),
        "UTC",
    )
    .unwrap();
    let Report::Cashflow(cash) = report else {
        panic!()
    };
    assert_eq!(
        codes(&cash.meta.insights),
        vec![InsightCode::Overspent, InsightCode::CostliestMonth]
    );
    assert_eq!(cash.meta.insights[1].subject.as_deref(), Some("2026-05"));
}

#[test]
fn the_overview_compares_the_month_with_the_one_before() {
    let (_dir, mut vault, card) = vault_with_card();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&card, "1000", "2026-05-05T10:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "400", "groceries", "2026-05-06T10:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&card, "1000", "2026-06-05T10:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "500", "groceries", "2026-06-06T10:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "100", "taxi", "2026-06-07T10:00:00Z"),
    )
    .unwrap();
    let view = overview(&vault);
    assert_eq!(view.month, "2026-06");
    assert_eq!(view.previous_month, "2026-05");
    assert_eq!(view.flow.spent, amount("600"));
    assert_eq!(view.flow.previous_spent, amount("400"));
    assert_eq!(view.flow.top_category.as_deref(), Some("groceries"));
    assert_eq!(
        codes(&view.flow.insights),
        vec![
            InsightCode::Saved,
            InsightCode::SpentUp,
            InsightCode::TopCategory
        ]
    );
    assert_eq!(view.flow.insights[1].pct, Some(amount("50")));
    assert!(view.flow.has_data);
    assert!(view.wealth.has_data);
    assert_eq!(view.wealth.net, amount("1000"));
    assert!(view.savings.has_data);
    assert_eq!(view.savings.saved, amount("1000"));
    assert!(!view.investments.has_data);
}

#[test]
fn an_empty_vault_has_no_data_on_any_card() {
    let (_dir, vault, _card) = vault_with_card();
    let view = overview(&vault);
    assert!(!view.flow.has_data);
    assert!(!view.wealth.has_data);
    assert!(!view.investments.has_data);
    assert!(!view.savings.has_data);
    assert!(view.flow.insights.is_empty());
}
