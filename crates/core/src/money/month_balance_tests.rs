//! Tests of the month_balance area: the old inline tests (names and bodies
//! unchanged), then the carry rule and the delegate pins.

use super::month_balance::*;
use crate::Amount;
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::service::test_support::*;
use crate::service::{ReportFilter, UnlockedVault};
use crate::time::unix_secs;

const NOW: &str = "2026-06-15T12:00:00Z";

fn dated(mut tx: Transaction, ts: &str) -> Transaction {
    at(&mut tx, ts);
    tx
}

fn balance(vault: &UnlockedVault, from: &str, to: &str) -> MonthBalance {
    let filter = ReportFilter {
        from: Some(from.into()),
        to: Some(to.into()),
        ..ReportFilter::default()
    };
    vault
        .month_balance(&filter, unix_secs(NOW).unwrap(), "UTC")
        .unwrap()
}

fn vault_with_months() -> (tempfile::TempDir, UnlockedVault) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    for (month, spent) in [("03", "700"), ("04", "1200"), ("05", "900"), ("06", "100")] {
        vault
            .add_transaction(dated(
                income(&card, "1000", "salary"),
                &format!("2026-{month}-05T10:00:00Z"),
            ))
            .unwrap();
        vault
            .add_transaction(dated(
                payment(&card, spent, "shop"),
                &format!("2026-{month}-10T10:00:00Z"),
            ))
            .unwrap();
    }
    (dir, vault)
}

#[test]
fn without_carry_each_month_stands_alone() {
    let (_dir, vault) = vault_with_months();
    let report = balance(&vault, "2026-03", "2026-06");
    let left: Vec<Amount> = report.rows.iter().map(|row| row.left).collect();
    assert_eq!(
        left,
        vec![amount("300"), amount("-200"), amount("100"), amount("900")]
    );
    assert!(report.rows.iter().all(|row| row.carried_in == Amount::ZERO));
    assert!(report.rows[3].current);
    // The running month is left out of the average: (700 + 1200 + 900) / 3.
    assert_eq!(report.average_spent, amount("933.33"));
    assert_eq!(report.total.left, amount("1100"));
    assert_eq!(report.total.month, "2026-06");
}

#[test]
fn the_carry_starts_where_it_was_turned_on_even_before_the_period() {
    let (_dir, mut vault) = vault_with_months();
    vault.set_carry_over(Some("2026-04")).unwrap();
    let report = balance(&vault, "2026-05", "2026-06");
    assert_eq!(report.carry_since.as_deref(), Some("2026-04"));
    // April −200 carries into May; May +100 on top makes −100 for June.
    assert_eq!(report.rows[0].carried_in, amount("-200"));
    assert_eq!(report.rows[0].total, amount("-100"));
    assert_eq!(report.rows[1].carried_in, amount("-100"));
    assert_eq!(report.rows[1].total, amount("800"));
    assert_eq!(report.total.total, amount("800"));
    vault.set_carry_over(None).unwrap();
    assert_eq!(
        balance(&vault, "2026-05", "2026-06").rows[1].carried_in,
        Amount::ZERO
    );
}

#[test]
fn the_last_month_goes_day_by_day_and_ends_at_its_total() {
    let (_dir, vault) = vault_with_months();
    let report = balance(&vault, "2026-03", "2026-06");
    assert_eq!(report.days.len(), 30);
    let balance_on = |day: usize| report.days[day - 1].balance;
    // Nothing before the salary on the 5th, +1000 then, −100 on the 10th.
    assert_eq!(balance_on(4), Some(Amount::ZERO));
    assert_eq!(balance_on(5), Some(amount("1000")));
    assert_eq!(balance_on(15), Some(report.total.total));
    assert_eq!(balance_on(16), None);
    // The chart labels the biggest income and the biggest spending.
    let income = report.income_mark.as_ref().expect("an income mark");
    assert_eq!(income.amount, amount("1000"));
    assert_eq!(income.date, "2026-06-05");
    assert_eq!(
        report.spent_mark.as_ref().map(|mark| mark.amount),
        Some(amount("100"))
    );
    let forecast: Vec<Amount> = report.days.iter().filter_map(|day| day.forecast).collect();
    // A forecast exists only with an allowance; it never rises.
    assert!(report.per_day.is_some() || forecast.is_empty());
    assert!(forecast.windows(2).all(|pair| pair[1] <= pair[0]));
}

#[test]
fn a_bad_month_is_refused() {
    let (_dir, mut vault) = vault_with_months();
    assert!(vault.set_carry_over(Some("June")).is_err());
}

#[test]
fn a_carry_month_is_normalized_and_blank_stops_it() {
    assert_eq!(
        parse_carry_since(Some(" 2026-04 ")).unwrap().as_deref(),
        Some("2026-04")
    );
    assert_eq!(parse_carry_since(Some("  ")).unwrap(), None);
    assert_eq!(parse_carry_since(None).unwrap(), None);
    assert!(parse_carry_since(Some("June")).is_err());
}

/// The reports area as a test fakes it: fixed flows, no vault.
struct FakeReports {
    flows: ReportFlows,
    allowance: Option<Amount>,
}

impl ReportSource for FakeReports {
    fn shown_months(&self) -> crate::error::Result<Vec<chrono::NaiveDate>> {
        Ok(self.flows.months.last().copied().into_iter().collect())
    }

    fn report(&self, _from: &str, _to: &str) -> crate::error::Result<ReportFlows> {
        Ok(self.flows.clone())
    }

    fn allowance(&self, _month: &str, _currency: &str) -> crate::error::Result<Option<Amount>> {
        Ok(self.allowance)
    }
}

fn date(text: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
}

#[test]
fn the_pure_balance_runs_on_a_fake_source_without_a_vault() {
    let (_dir, vault) = empty_vault();
    let fake = FakeReports {
        flows: ReportFlows {
            currency: "USD".into(),
            today: date("2026-06-15"),
            months: vec![date("2026-06-01")],
            cash: vec![CashMonth {
                month: "2026-06".into(),
                income: amount("1000"),
                fixed: amount("100"),
                daily: amount("200"),
            }],
            last_month: vec![
                DayFlow {
                    date: date("2026-06-05"),
                    value: amount("1000"),
                    kind: FlowKind::Income,
                    category: "salary".into(),
                },
                DayFlow {
                    date: date("2026-06-10"),
                    value: amount("300"),
                    kind: FlowKind::Spending,
                    category: "shop".into(),
                },
            ],
        },
        allowance: Some(amount("10")),
    };
    let report = month_balance(&vault.books(), &fake).unwrap();
    assert_eq!(report.currency, "USD");
    assert_eq!(report.rows.len(), 1);
    assert_eq!(report.rows[0].left, amount("700"));
    assert!(report.rows[0].current);
    assert_eq!(report.carry_since, None);
    assert_eq!(report.per_day, Some(amount("10")));
    assert_eq!(report.days.len(), 30);
    assert_eq!(report.days[3].balance, Some(Amount::ZERO));
    assert_eq!(report.days[14].balance, Some(amount("700")));
    assert_eq!(report.days[15].balance, None);
    assert_eq!(report.days[15].forecast, Some(amount("690")));
    assert_eq!(
        report.spent_mark.map(|mark| mark.category),
        Some("shop".into())
    );
}

/// The vault of `service/golden_tests.rs` (BTC bought twice, sold in February)
/// with a salary and a payment next to it.
fn golden_vault() -> (tempfile::TempDir, UnlockedVault) {
    let (dir, mut vault) = setup();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let store_id = vault.doc.stores[1].id.clone();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    for (tx, stamp) in [
        (
            income_of(&store_id, btc(), "1", "40000", "buy"),
            "2026-01-01T00:00:00Z",
        ),
        (
            income_of(&store_id, btc(), "1", "50000", "buy"),
            "2026-01-02T00:00:00Z",
        ),
        (
            sell_btc(&store_id, "1", "60000", "60000"),
            "2026-02-03T00:00:00Z",
        ),
        (income(&card, "1000", "salary"), "2026-02-05T00:00:00Z"),
        (payment(&card, "250", "shop"), "2026-02-10T00:00:00Z"),
    ] {
        vault.add_transaction(dated(tx, stamp)).unwrap();
    }
    (dir, vault)
}

#[test]
fn the_golden_vault_balance_is_pinned_and_the_delegate_equals_the_scenario() {
    let (_dir, vault) = golden_vault();
    let filter = ReportFilter {
        from: Some("2026-01".into()),
        to: Some("2026-02".into()),
        ..ReportFilter::default()
    };
    let now = unix_secs(NOW).unwrap();
    let report = vault.month_balance(&filter, now, "UTC").unwrap();
    assert_eq!(
        report,
        crate::usecases::household::month_balance::month_balance(&vault, &filter, now, "UTC")
            .unwrap()
    );
    let months: Vec<&str> = report.rows.iter().map(|row| row.month.as_str()).collect();
    assert_eq!(months, vec!["2026-01", "2026-02"]);
    let left: Vec<Amount> = report.rows.iter().map(|row| row.left).collect();
    assert_eq!(left, vec![amount("0"), amount("750")]);
}

#[test]
fn the_carry_delegate_equals_the_scenario_and_a_no_op_writes_nothing() {
    let (_dir, mut vault) = empty_vault();
    vault.set_carry_over(Some("2026-04")).unwrap();
    assert_eq!(vault.doc.settings.carry_since.as_deref(), Some("2026-04"));
    let written = std::fs::read(&vault.path).unwrap();
    // The same month again, and the scenario itself: nothing is saved.
    vault.set_carry_over(Some(" 2026-04 ")).unwrap();
    crate::usecases::household::month_balance::set_carry_over(&mut vault, Some("2026-04")).unwrap();
    assert_eq!(std::fs::read(&vault.path).unwrap(), written);
    // A different month is a write and the scenario reaches the same state.
    let (_twin_dir, mut twin) = empty_vault();
    crate::usecases::household::month_balance::set_carry_over(&mut twin, Some("2026-05")).unwrap();
    vault.set_carry_over(Some("2026-05")).unwrap();
    assert_ne!(std::fs::read(&vault.path).unwrap(), written);
    assert_eq!(
        vault.doc.settings.carry_since,
        twin.doc.settings.carry_since
    );
}
