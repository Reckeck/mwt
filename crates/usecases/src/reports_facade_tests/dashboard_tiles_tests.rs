//! Tests of the dashboard_tiles area: the old inline tests (names and bodies unchanged).

use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::model::vault::DashPeriod;
use crate::service::test_support::*;
use crate::time::unix_secs;

fn dated(mut tx: Transaction, category: &str, ts: &str) -> Transaction {
    tx.meta_mut().categories = vec![category.into()];
    at(&mut tx, ts);
    tx
}

#[test]
fn the_month_pace_compares_with_the_same_date_and_names_growth() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    for tx in [
        dated(
            income(&card, "1000", "pay"),
            "salary",
            "2026-09-05T10:00:00Z",
        ),
        dated(
            payment(&card, "100", "food"),
            "groceries",
            "2026-09-06T10:00:00Z",
        ),
        dated(payment(&card, "50", "taxi"), "taxi", "2026-09-07T10:00:00Z"),
        // After the same date last month: not in the base.
        dated(
            payment(&card, "300", "late"),
            "groceries",
            "2026-09-20T10:00:00Z",
        ),
        dated(
            income(&card, "1000", "pay"),
            "salary",
            "2026-10-05T10:00:00Z",
        ),
        dated(
            payment(&card, "180", "food"),
            "groceries",
            "2026-10-06T10:00:00Z",
        ),
        dated(payment(&card, "60", "taxi"), "taxi", "2026-10-07T10:00:00Z"),
        dated(
            payment(&card, "20", "cinema"),
            "entertainment",
            "2026-10-08T10:00:00Z",
        ),
    ] {
        crate::usecases::money::transactions::add_transaction(&mut vault, tx).unwrap();
    }
    let now = unix_secs("2026-10-10T12:00:00Z").unwrap();
    let pace = crate::reports::dashboard_tiles::month_pace(&vault.books(), now, "UTC");
    assert_eq!(pace.month, "2026-10");
    assert_eq!((pace.day, pace.days), (10, 31));
    assert_eq!(pace.income.base, Some(amount("1000")));
    assert_eq!(pace.spent.value, amount("260"));
    assert_eq!(pace.spent.base, Some(amount("150")));
    assert_eq!(pace.saved.value, amount("740"));
    // 260 over 10 of 31 days.
    assert_eq!(pace.forecast_spent, amount("806"));
    let keys: Vec<&str> = pace
        .growth
        .iter()
        .map(|row| row.category.as_str())
        .collect();
    assert_eq!(keys, ["groceries", "entertainment", "taxi"]);
    assert!(pace.spend_plan_to_date.is_some());
}

#[test]
fn a_new_vault_has_no_base_and_no_growth() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        dated(
            payment(&card, "40", "food"),
            "groceries",
            "2026-10-03T10:00:00Z",
        ),
    )
    .unwrap();
    let pace = crate::reports::dashboard_tiles::month_pace(
        &vault.books(),
        unix_secs("2026-10-10T12:00:00Z").unwrap(),
        "UTC",
    );
    assert_eq!(pace.spent.base, None);
    assert!(pace.growth.is_empty());
    assert_eq!(pace.spend_plan_to_date, None);
}

#[test]
fn the_capital_trend_starts_from_the_value_before_the_period() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    for (sum, ts) in [
        ("500", "2026-09-20T10:00:00Z"),
        ("200", "2026-10-04T10:00:00Z"),
    ] {
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            dated(income(&card, sum, "pay"), "salary", ts),
        )
        .unwrap();
    }
    let trend = crate::reports::dashboard_tiles::net_worth_trend(
        &vault.books(),
        DashPeriod::Month,
        unix_secs("2026-10-10T12:00:00Z").unwrap(),
        "UTC",
    );
    assert_eq!(trend.start, "2026-10-01");
    assert_eq!(trend.series.len(), 10);
    assert_eq!(trend.change.base, Some(amount("500")));
    assert_eq!(trend.change.value, amount("700"));
    assert_eq!(trend.series[0], amount("500"));
    assert_eq!(trend.series[3], amount("700"));
}

mod rate_change_tests {
    use crate::model::vault::{FxPoint, PricePoint};
    use crate::service::test_support::*;

    #[test]
    fn rates_and_prices_carry_their_change() {
        let (_dir, mut vault) = setup();
        vault.set_base_currency("usd");
        for (per_usd, ts) in [
            ("100", "2026-10-20T10:00:00Z"),
            ("80", "2026-10-21T12:00:00Z"),
        ] {
            vault.doc.fx_cache.push(FxPoint {
                currency: "rub".into(),
                per_usd: amount(per_usd),
                ts: ts.into(),
                source: "test".into(),
            });
        }
        let table = crate::usecases::household::losses::fx_table(&vault, &[]);
        let rub = table.rows.iter().find(|row| row.currency == "rub").unwrap();
        // One rouble went from 0.01 to 0.0125 dollars.
        assert_eq!(rub.change_pct, Some(amount("25")));

        let key = btc().key();
        vault.doc.price_cache.push(PricePoint {
            asset_key: key.clone(),
            value_usd: amount("50000"),
            ts: "2026-10-21T12:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: Some(amount("-3.5")),
        });
        let summary = crate::usecases::money::capital::portfolio_summary(&vault.books());
        if let Some(stat) = summary.assets.iter().find(|stat| stat.asset.key() == key) {
            assert_eq!(stat.change_24h_pct, Some(amount("-3.5")));
        }
    }
}
