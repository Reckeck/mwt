//! Tests of the inflation area that need the vault (`UnlockedVault`): moved from
//! `money/inflation_tests.rs` with their assertions unchanged (crate-split T13).

use crate::Amount;
use crate::model::deposit::KeyRatePoint;
use crate::model::inflation::{InflationMode, InflationReading, InflationSource};
use crate::model::notify::NoticeCategory;
use crate::money::inflation::*;
use crate::service::UnlockedVault;
use crate::service::test_support::{amount, empty_vault};
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;

fn reading(currency: &str, month: &str, yoy: &str) -> InflationReading {
    InflationReading {
        currency: currency.into(),
        month: month.into(),
        yoy_pct: amount(yoy),
        source: InflationSource::for_currency(currency).unwrap_or(InflationSource::Cbr),
    }
}

fn day(raw: &str) -> NaiveDate {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
}

fn table(pairs: &[(&str, &str)]) -> BTreeMap<String, Amount> {
    pairs
        .iter()
        .map(|(code, rate)| ((*code).to_string(), amount(rate)))
        .collect()
}

#[test]
fn mode_is_inferred_until_chosen() {
    let (_dir, mut vault) = empty_vault();
    assert_eq!(vault.inflation_mode("RUB"), InflationMode::Auto);
    // No built-in source for THB.
    assert_eq!(vault.inflation_mode("thb"), InflationMode::Manual);
    set_rates(&mut vault, &[("rub", "9.5")]);
    assert_eq!(vault.inflation_mode("rub"), InflationMode::Manual);
    vault
        .set_inflation_mode("rub", Some(InflationMode::Auto))
        .unwrap();
    assert_eq!(vault.inflation_mode("rub"), InflationMode::Auto);
    vault.set_inflation_mode("rub", None).unwrap();
    assert_eq!(vault.inflation_mode("rub"), InflationMode::Manual);
    assert!(
        vault
            .set_inflation_mode("thb", Some(InflationMode::Auto))
            .is_err()
    );
}

#[test]
fn auto_reads_the_series_and_falls_back_to_the_flat_rate() {
    let (_dir, mut vault) = empty_vault();
    assert_eq!(vault.inflation_on("rub", "2026-09"), Some(amount("8")));
    vault.apply_inflation(&[
        reading("rub", "2026-07", "5.98"),
        reading("rub", "2026-09", "6.35"),
    ]);
    assert_eq!(vault.inflation_on("rub", "2026-09"), Some(amount("6.35")));
    // A month without a reading takes the one before it…
    assert_eq!(vault.inflation_on("rub", "2026-08"), Some(amount("5.98")));
    // …and a month before the series its first reading.
    assert_eq!(vault.inflation_on("rub", "2020-01"), Some(amount("5.98")));
    assert_eq!(
        vault.effective_inflation().get("rub"),
        Some(&amount("6.35"))
    );
    vault
        .set_inflation_mode("rub", Some(InflationMode::Manual))
        .unwrap();
    assert_eq!(vault.inflation_on("rub", "2026-09"), Some(amount("8")));
    assert_eq!(vault.effective_inflation().get("thb"), Some(&amount("1")));
}

#[test]
fn apply_reports_only_new_months() {
    let (_dir, mut vault) = empty_vault();
    // The first fetch fills history: nothing to announce.
    let news = vault.apply_inflation(&[
        reading("rub", "2026-07", "5.98"),
        reading("rub", "2026-08", "6.33"),
    ]);
    assert!(news.is_empty());
    // A revision of a known month replaces it silently.
    let news = vault.apply_inflation(&[reading("rub", "2026-08", "6.30")]);
    assert!(news.is_empty());
    assert_eq!(vault.inflation_series("rub").len(), 2);
    assert_eq!(vault.inflation_series("rub")[1].yoy_pct, amount("6.30"));
    let news = vault.apply_inflation(&[reading("RUB", "2026-09", "6.35")]);
    assert_eq!(news.len(), 1);
    assert_eq!(news[0].month, "2026-09");
    assert_eq!(news[0].previous, Some(amount("6.30")));
    assert_eq!(vault.inflation_series("rub").len(), 3);
}

#[test]
fn rates_notices_only_for_news() {
    let (_dir, mut vault) = empty_vault();
    let key = |date: &str, rate: &str| KeyRatePoint {
        date: date.into(),
        rate: amount(rate),
    };
    let first = FetchedRates {
        key_rates: Some(vec![key("2026-07-28", "14")]),
        inflation: vec![reading("rub", "2026-08", "6.33")],
    };
    assert!(vault.apply_rates(&first, true).unwrap().is_empty());
    let second = FetchedRates {
        key_rates: Some(vec![key("2026-07-28", "14"), key("2026-10-24", "13")]),
        inflation: vec![
            reading("rub", "2026-08", "6.33"),
            reading("rub", "2026-09", "6.35"),
        ],
    };
    let notices = vault.apply_rates(&second, true).unwrap();
    let bodies: Vec<&str> = notices.iter().map(|notice| notice.body.as_str()).collect();
    assert_eq!(
        bodies,
        [
            "ЦБ изменил ставку: 14% -> 13% с 2026-10-24",
            "Инфляция (RUB) в сентябре 6.35% (+0.02)",
        ]
    );
    assert!(
        notices
            .iter()
            .all(|notice| notice.category == NoticeCategory::Rates)
    );
    // Fetching the same again says nothing.
    assert!(vault.apply_rates(&second, false).unwrap().is_empty());
}

#[test]
fn a_constant_series_matches_the_flat_rate() {
    let (_dir, mut vault) = empty_vault();
    let today = day("2026-10-21");
    let rows: Vec<InflationReading> = (1..=12)
        .map(|month| reading("rub", &format!("2025-{month:02}"), "8"))
        .collect();
    vault.apply_inflation(&rows);
    let table = vault
        .real_coefficients("rub", day("2024-10-01"), today)
        .unwrap();
    assert_eq!(table[&day("2026-10-01")], Amount::from(1));
    for (month, coefficient) in &table {
        let elapsed =
            (today.year() - month.year()) * 12 + today.month() as i32 - month.month() as i32;
        let flat = Amount::from_f64(1.08_f64.powf(f64::from(elapsed) / 12.0));
        assert!(
            (*coefficient - flat).abs() < amount("0.01"),
            "{month}: {coefficient} vs {flat}"
        );
    }
    assert!((table[&day("2025-10-01")] - amount("1.08")).abs() < amount("0.0001"));
}

#[test]
fn coefficients_follow_the_months() {
    let (_dir, mut vault) = empty_vault();
    vault.apply_inflation(&[
        reading("rub", "2026-08", "12"),
        reading("rub", "2026-09", "0"),
    ]);
    let table = vault
        .real_coefficients("rub", day("2026-08-01"), day("2026-10-05"))
        .unwrap();
    // September at 0 % and October carrying it: September's money kept its value.
    assert_eq!(table[&day("2026-09-01")], Amount::from(1));
    assert_eq!(table[&day("2026-08-01")], Amount::from(1));
    let table = vault
        .real_coefficients("rub", day("2026-07-01"), day("2026-09-05"))
        .unwrap();
    // August at 12 %: one month of it on July's money.
    let month_of_twelve = amount("1.12").root_n(12);
    assert_eq!(table[&day("2026-07-01")], month_of_twelve);
    assert!(
        vault
            .real_coefficients("thb", day("2026-07-01"), day("2026-09-05"))
            .is_none()
    );
}

fn set_rates(vault: &mut UnlockedVault, pairs: &[(&str, &str)]) {
    vault
        .update_settings(crate::ipc_contract::UpdateSettingsInput {
            inflation_manual: Some(table(pairs)),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn the_flat_table_is_the_shipped_one_under_the_vaults_own_rates() {
    let (_dir, mut vault) = empty_vault();
    assert_eq!(
        vault.flat_inflation(),
        crate::appconfig::default_inflation()
    );
    set_rates(&mut vault, &[("rub", "9.5"), ("gbp", "4")]);
    let flat = vault.flat_inflation();
    assert_eq!(flat.get("rub"), Some(&amount("9.5")));
    assert_eq!(flat.get("gbp"), Some(&amount("4")));
    assert_eq!(flat.get("usd"), Some(&amount("3")));
    assert_eq!(flat.get("thb"), Some(&amount("1")));
}

#[test]
fn a_saved_table_keeps_only_what_differs_or_was_already_set() {
    let (_dir, mut vault) = empty_vault();
    // The form sends the whole table; the shipped values in it are not data.
    set_rates(&mut vault, &[("rub", "8"), ("usd", "3.5"), ("GBP", "4")]);
    assert_eq!(
        vault.doc.settings.inflation_manual,
        table(&[("usd", "3.5"), ("gbp", "4")])
    );
    // A value set once stays even when it is put back to the shipped one.
    set_rates(&mut vault, &[("usd", "3"), ("gbp", "4")]);
    assert_eq!(
        vault.doc.settings.inflation_manual,
        table(&[("usd", "3"), ("gbp", "4")])
    );
    // A rate that is not a number of percent is dropped.
    set_rates(&mut vault, &[("gbp", "4"), ("chf", "5000")]);
    assert_eq!(vault.doc.settings.inflation_manual, table(&[("gbp", "4")]));
}

#[test]
fn old_device_rates_are_adopted_once_and_never_overwrite() {
    let (_dir, mut vault) = empty_vault();
    set_rates(&mut vault, &[("gbp", "4")]);
    let local = table(&[("rub", "8"), ("usd", "5"), ("gbp", "9"), ("thb", "1")]);
    assert!(vault.adopt_flat_inflation(&local).unwrap());
    assert!(vault.doc.settings.inflation_adopted);
    // Only the differing, not yet set values moved over.
    assert_eq!(
        vault.doc.settings.inflation_manual,
        table(&[("usd", "5"), ("gbp", "4")])
    );
    // Once: a later change of the old table is not picked up again.
    let again = table(&[("usd", "6"), ("eur", "7")]);
    assert!(!vault.adopt_flat_inflation(&again).unwrap());
    assert_eq!(
        vault.doc.settings.inflation_manual,
        table(&[("usd", "5"), ("gbp", "4")])
    );
}

#[test]
fn adoption_survives_a_reopen_and_does_not_write_twice() {
    let (_dir, mut vault) = empty_vault();
    let local = table(&[("rub", "10")]);
    assert!(vault.adopt_flat_inflation(&local).unwrap());
    let reopened = UnlockedVault::open(vault.path.clone(), "pw").unwrap();
    assert!(reopened.doc.settings.inflation_adopted);
    assert_eq!(reopened.flat_inflation().get("rub"), Some(&amount("10")));
}
