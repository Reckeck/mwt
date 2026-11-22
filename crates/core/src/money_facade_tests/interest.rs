//! Tests of the interest area that need the vault (`UnlockedVault`): moved from
//! `money/interest_tests.rs` with their assertions unchanged (crate-split T13).

use crate::Amount;
use crate::model::deposit::{
    Capitalization, DepositTerms, IncomeBracket, KeyRatePoint, PdsTerms, RatePeriod,
};
use crate::model::store::{AssetRef, Binding, StoreKind, SubType};
use crate::money::interest::*;
use crate::service::StoreExtras;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use chrono::NaiveDate;

fn date(raw: &str) -> NaiveDate {
    day_of(raw).unwrap()
}

fn terms(rates: &[(&str, &str)]) -> DepositTerms {
    DepositTerms {
        rates: rates
            .iter()
            .map(|(from, rate)| RatePeriod {
                from: (*from).into(),
                rate: amount_str(rate),
            })
            .collect(),
        capitalization: Capitalization::Monthly,
        opened_at: None,
        term_months: None,
        auto_accrue: true,
        pds: None,
    }
}

fn amount_str(raw: &str) -> Amount {
    Amount::parse(raw).unwrap()
}

/// A ruble savings account with no money in it.
fn empty_savings(vault: &mut UnlockedVault, sub_type: SubType) -> String {
    let bank = store(vault, "Bank", StoreKind::Bank, None);
    vault
        .create_store_with(
            "Savings",
            StoreKind::Bank,
            "#000",
            "",
            Some(bank),
            StoreExtras {
                sub_type: Some(sub_type),
                binding: Some(Binding::Asset(AssetRef::fiat("RUB", "RUB"))),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id
}

/// A ruble savings account with 100 000 ₽ from the 1st of January 2026.
fn savings(vault: &mut UnlockedVault, sub_type: SubType) -> String {
    let account = empty_savings(vault, sub_type);
    let mut seed = income_of(
        &account,
        AssetRef::fiat("RUB", "RUB"),
        "100000",
        "0.01",
        "salary",
    );
    seed.meta_mut().timestamp = "2026-01-01T00:00:00Z".into();
    vault.add_transaction(seed).unwrap();
    account
}

#[test]
fn interest_follows_the_rate_history_day_by_day() {
    let (_dir, mut vault) = empty_vault();
    let account = savings(&mut vault, SubType::Savings);
    // 18.25 % for January's 31 days, then 0 %.
    vault
        .set_deposit_terms(
            &account,
            Some(terms(&[("2026-01-01", "18.25"), ("2026-02-01", "0")])),
        )
        .unwrap();
    let report = vault.deposit_report(&account, date("2026-03-10")).unwrap();
    // 100 000 × 18.25 % / 365 = 50 ₽ a day × 31 days.
    assert_eq!(report.accrued_year, amount_str("1550"));
    assert_eq!(report.rate, Some(amount_str("0")));
    assert_eq!(report.rate_since.as_deref(), Some("2026-02-01"));
}

#[test]
fn last_month_is_posted_once() {
    let (_dir, mut vault) = empty_vault();
    let account = savings(&mut vault, SubType::Savings);
    vault
        .set_deposit_terms(&account, Some(terms(&[("2026-01-01", "18.25")])))
        .unwrap();
    assert_eq!(vault.post_due_interest(date("2026-02-03")).unwrap(), 1);
    assert_eq!(vault.post_due_interest(date("2026-02-20")).unwrap(), 0);
    let report = vault.deposit_report(&account, date("2026-02-03")).unwrap();
    assert_eq!(report.posted_year, amount_str("1550"));
    assert_eq!(report.balance, amount_str("101550"));
}

#[test]
fn terms_belong_to_savings_only() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    assert!(
        vault
            .set_deposit_terms(&card, Some(terms(&[("2026-01-01", "10")])))
            .is_err()
    );
    let account = savings(&mut vault, SubType::Deposit);
    assert!(
        vault
            .set_deposit_terms(&account, Some(terms(&[("2026-13-01", "10")])))
            .is_err()
    );
    assert!(
        vault
            .set_deposit_terms(&account, Some(terms(&[("2026-01-01", "150")])))
            .is_err()
    );
}

#[test]
fn pds_cofinancing_follows_the_bracket_and_the_cap() {
    let (_dir, mut vault) = empty_vault();
    let account = savings(&mut vault, SubType::Pds);
    let mut pds_terms = terms(&[]);
    pds_terms.pds = Some(PdsTerms {
        first_contribution_at: "2026-01-01".into(),
        income_bracket: IncomeBracket::Mid,
    });
    vault.set_deposit_terms(&account, Some(pds_terms)).unwrap();
    let pds = vault
        .deposit_report(&account, date("2026-06-01"))
        .unwrap()
        .pds
        .unwrap();
    // 100 000 ₽ in at 1:2 → 50 000, capped at 36 000.
    assert_eq!(pds.contributions, amount_str("100000"));
    assert_eq!(pds.cofinancing, amount_str("36000"));
    assert_eq!(pds.ratio, 2);
    assert_eq!(pds.ends_at, "2036-01-01");
    assert_eq!(pds.cofinancing_due, "2027-08-31");
    assert_eq!(pds.deduction, amount_str("13000"));
}

#[test]
fn interest_tax_uses_the_highest_key_rate_of_the_year() {
    let (_dir, mut vault) = empty_vault();
    let account = savings(&mut vault, SubType::Savings);
    vault.apply_key_rates(&[
        KeyRatePoint {
            date: "2025-12-20".into(),
            rate: amount_str("16"),
        },
        KeyRatePoint {
            date: "2026-03-20".into(),
            rate: amount_str("21"),
        },
        KeyRatePoint {
            date: "2026-08-01".into(),
            rate: amount_str("17"),
        },
    ]);
    let mut paid = income_of(
        &account,
        AssetRef::fiat("RUB", "RUB"),
        "250000",
        "0.01",
        "interest",
    );
    paid.meta_mut().categories = vec![INTEREST_CATEGORY.into()];
    paid.meta_mut().timestamp = "2026-06-30T00:00:00Z".into();
    vault.add_transaction(paid).unwrap();
    let tax = vault.interest_tax(2026);
    // The highest rate on a 1st of a month is 21 % (April–August).
    assert_eq!(tax.max_key_rate, Some(amount_str("21")));
    assert_eq!(tax.limit, Some(amount_str("210000")));
    assert_eq!(tax.taxable, amount_str("40000"));
    assert_eq!(tax.tax, amount_str("5200"));
}

/// L14: 01:00 of the 10th in UTC+3 is still the 9th in UTC. The user's day is
/// the 10th, so the deposit earns from the 10th, not from the 9th.
#[test]
fn a_deposit_at_one_in_the_morning_in_utc_plus_three_accrues_no_extra_day() {
    let (_dir, mut vault) = empty_vault();
    vault.doc.settings.schedule_timezone = Some("Europe/Moscow".into());
    let account = empty_savings(&mut vault, SubType::Savings);
    vault
        .set_deposit_terms(&account, Some(terms(&[("2026-01-01", "18.25")])))
        .unwrap();
    let mut deposit = income_of(
        &account,
        AssetRef::fiat("RUB", "RUB"),
        "100000",
        "0.01",
        "salary",
    );
    deposit.meta_mut().timestamp = "2026-01-09T22:00:00Z".into();
    vault.add_transaction(deposit).unwrap();
    let report = vault.deposit_report(&account, date("2026-01-10")).unwrap();
    // 100 000 × 18.25 % / 365 = 50 ₽ a day, for the 10th only.
    assert_eq!(report.accrued_year, amount_str("50"));
}
