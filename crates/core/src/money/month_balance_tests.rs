//! Tests of the month_balance area: the old inline tests (names and bodies
//! unchanged), then the carry rule and the delegate pins.

use super::month_balance::*;
use crate::Amount;
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::pricing::history::unix_secs;
use crate::service::UnlockedVault;
use crate::service::reports::ReportFilter;
use crate::service::test_support::*;

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
