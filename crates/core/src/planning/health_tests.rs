//! Tests of the capital checkup: the old inline tests (names and bodies unchanged).

use super::health::{SAVINGS_MONTHS, SavingsMonth, savings_figures};
use crate::Amount;
use crate::model::planning::EmergencyFund;
use crate::model::planning::{HealthColor, HealthReasonCode};
use crate::model::store::{StoreKind, StoreScope, SubType};
use crate::model::transaction::Transaction;
use crate::pricing::history::secs_or_now;
use crate::service::StoreExtras;
use crate::service::test_support::*;
use crate::service::{Insight, InsightCode, LoanClass, UnlockedVault};
use crate::usecases::planning::health::*;

fn pct(raw: &str) -> Amount {
    Amount::parse(raw).unwrap()
}

fn base_facts() -> HealthFacts {
    HealthFacts {
        has_history: true,
        fund_set: true,
        fund_total: pct("600"),
        e_mand: Some(pct("100")),
        months_covered: Some(pct("6")),
        savings_avg_pct: Some(pct("20")),
        savings_target_pct: pct("10"),
        ..HealthFacts::default()
    }
}

fn codes(facts: &HealthFacts) -> (HealthColor, Vec<HealthReasonCode>) {
    let (color, reasons) = judge(facts);
    (color, reasons.into_iter().map(|item| item.code).collect())
}

#[test]
fn healthy_facts_are_green_without_reasons() {
    assert_eq!(codes(&base_facts()), (HealthColor::Green, vec![]));
}

#[test]
fn dti_boundaries_are_strict() {
    let mut facts = base_facts();
    facts.dti_pct = Some(pct("30"));
    assert_eq!(codes(&facts).0, HealthColor::Green);
    facts.dti_pct = Some(pct("30.1"));
    assert_eq!(
        codes(&facts),
        (HealthColor::Yellow, vec![HealthReasonCode::DtiElevated])
    );
    facts.dti_pct = Some(pct("50"));
    assert_eq!(codes(&facts).0, HealthColor::Yellow);
    facts.dti_pct = Some(pct("50.1"));
    assert_eq!(
        codes(&facts),
        (HealthColor::Red, vec![HealthReasonCode::DtiHigh])
    );
}

#[test]
fn fund_boundaries_and_gap() {
    let mut facts = base_facts();
    facts.months_covered = Some(pct("3"));
    facts.fund_total = pct("300");
    assert_eq!(codes(&facts).0, HealthColor::Green);
    facts.months_covered = Some(pct("2.5"));
    facts.fund_total = pct("250");
    let (color, reasons) = judge(&facts);
    assert_eq!(color, HealthColor::Yellow);
    assert_eq!(reasons[0].code, HealthReasonCode::FundBelowThree);
    assert_eq!(reasons[0].amount, Some(pct("50")));
    facts.months_covered = Some(pct("0.5"));
    facts.fund_total = pct("50");
    let (color, reasons) = judge(&facts);
    assert_eq!(color, HealthColor::Red);
    assert_eq!(reasons[0].code, HealthReasonCode::FundBelowOne);
    assert_eq!(reasons[0].amount, Some(pct("50")));
    facts.months_covered = Some(pct("1"));
    assert_eq!(codes(&facts).1, vec![HealthReasonCode::FundBelowThree]);
}

#[test]
fn a_missing_fund_is_yellow_not_red() {
    let mut facts = base_facts();
    facts.fund_set = false;
    facts.months_covered = None;
    assert_eq!(
        codes(&facts),
        (HealthColor::Yellow, vec![HealthReasonCode::FundNotSet])
    );
}

#[test]
fn no_history_is_no_data_but_a_costly_card_is_still_red() {
    let facts = HealthFacts::default();
    assert_eq!(
        codes(&facts),
        (HealthColor::NoData, vec![HealthReasonCode::NoHistory])
    );
    let facts = HealthFacts {
        loans: vec![("card".into(), LoanClass::Costly, pct("1000"), None)],
        ..HealthFacts::default()
    };
    let (color, reasons) = judge(&facts);
    assert_eq!(color, HealthColor::Red);
    assert_eq!(reasons[0].subject.as_deref(), Some("card"));
    assert_eq!(reasons[1].code, HealthReasonCode::NoHistory);
}

#[test]
fn hints_do_not_change_the_colour() {
    let mut facts = base_facts();
    facts.loans = vec![("loan".into(), LoanClass::Unknown, pct("10"), None)];
    facts.creep = Some(Insight {
        code: InsightCode::LifestyleCreep,
        amount: Some(pct("50")),
        pct: Some(pct("100")),
        subject: Some("year".into()),
    });
    assert_eq!(
        codes(&facts),
        (
            HealthColor::Green,
            vec![
                HealthReasonCode::LoanWithoutRate,
                HealthReasonCode::LifestyleCreep
            ]
        )
    );
}

#[test]
fn savings_below_plan_is_yellow_and_sorted_after_red() {
    let mut facts = base_facts();
    facts.savings_avg_pct = Some(pct("5"));
    facts.savings_gap = Some(pct("25"));
    facts.dti_pct = Some(pct("60"));
    assert_eq!(
        codes(&facts),
        (
            HealthColor::Red,
            vec![
                HealthReasonCode::DtiHigh,
                HealthReasonCode::SavingsBelowPlan
            ]
        )
    );
}

fn dated(mut tx: Transaction, ts: &str) -> Transaction {
    at(&mut tx, ts);
    tx
}

/// Income and spending for each `(month, income, spent)`.
fn months(vault: &mut UnlockedVault, store: &str, rows: &[(&str, &str, &str)]) {
    for (month, earned, spent) in rows {
        vault
            .add_transaction(dated(
                income(store, earned, "pay"),
                &format!("{month}-05T10:00:00Z"),
            ))
            .unwrap();
        vault
            .add_transaction(dated(
                payment(store, spent, "food"),
                &format!("{month}-10T10:00:00Z"),
            ))
            .unwrap();
    }
}

#[test]
fn creep_compares_with_the_same_months_a_year_back() {
    let (_dir, mut vault) = empty_vault();
    quote_usd(&mut vault);
    vault.set_base_currency("usd");
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    months(
        &mut vault,
        &cash,
        &[
            ("2025-03", "1000", "800"),
            ("2025-04", "1000", "800"),
            ("2025-05", "1000", "800"),
            ("2026-03", "1200", "1000"),
            ("2026-04", "1200", "1000"),
            ("2026-05", "1200", "1000"),
        ],
    );
    let now = secs_or_now("2026-06-15T10:00:00Z");
    let creep = vault.lifestyle_creep(now, "UTC").expect("creep");
    assert_eq!(creep.subject.as_deref(), Some("year"));
    assert_eq!(creep.amount, Some(pct("200")));
    assert_eq!(creep.pct, Some(pct("100")));
}

#[test]
fn creep_without_a_year_takes_the_months_before() {
    let (_dir, mut vault) = empty_vault();
    quote_usd(&mut vault);
    vault.set_base_currency("usd");
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    months(
        &mut vault,
        &cash,
        &[
            ("2026-01", "1000", "500"),
            ("2026-02", "1000", "500"),
            ("2026-03", "1000", "500"),
            ("2026-04", "1100", "550"),
            ("2026-05", "1100", "550"),
            ("2026-06", "1100", "550"),
        ],
    );
    let now = secs_or_now("2026-07-02T10:00:00Z");
    assert_eq!(vault.lifestyle_creep(now, "UTC"), None, "spent grew slower");
    for month in ["2026-04", "2026-05", "2026-06"] {
        vault
            .add_transaction(dated(
                payment(&cash, "100", "extra"),
                &format!("{month}-20T10:00:00Z"),
            ))
            .unwrap();
    }
    let creep = vault.lifestyle_creep(now, "UTC").expect("creep");
    assert_eq!(creep.subject.as_deref(), Some("previous"));
    assert_eq!(creep.amount, Some(pct("150")));
}

#[test]
fn the_checkup_reads_the_vault_and_notices_only_changes() {
    let (_dir, mut vault) = empty_vault();
    quote_usd(&mut vault);
    vault.set_base_currency("usd");
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let start = secs_or_now("2026-04-02T10:00:00Z");
    let report = vault.capital_health(start, "UTC").unwrap();
    assert_eq!(report.color, HealthColor::NoData);
    assert_eq!(vault.health_notice(start, "UTC", true).unwrap(), None);
    assert_eq!(
        vault.doc.health_last.as_ref().map(|last| last.color),
        Some(HealthColor::NoData)
    );

    months(
        &mut vault,
        &cash,
        &[
            ("2026-02", "1000", "950"),
            ("2026-03", "1000", "950"),
            ("2026-04", "1000", "950"),
        ],
    );
    let may = secs_or_now("2026-05-03T10:00:00Z");
    let report = vault.capital_health(may, "UTC").unwrap();
    assert_eq!(report.savings.len(), 12);
    assert_eq!(report.savings_avg_pct, Some(pct("5")));
    assert_eq!(report.color, HealthColor::Yellow);
    let found: Vec<HealthReasonCode> = report.reasons.iter().map(|item| item.code).collect();
    assert_eq!(
        found,
        vec![
            HealthReasonCode::FundNotSet,
            HealthReasonCode::SavingsBelowPlan
        ]
    );
    let sent = vault
        .health_notice(may, "UTC", true)
        .unwrap()
        .expect("sent");
    assert_eq!(sent.key, "health:2026-05");
    assert_eq!(sent.link.as_deref(), Some(crate::routes::MONEY_HEALTH));
    assert!(sent.body.contains("жёлтый"), "{}", sent.body);
    assert_eq!(vault.health_notice(may, "UTC", true).unwrap(), None);

    let june = secs_or_now("2026-06-03T10:00:00Z");
    months(&mut vault, &cash, &[("2026-05", "1000", "950")]);
    assert_eq!(vault.health_notice(june, "UTC", true).unwrap(), None);

    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    let extras = StoreExtras {
        sub_type: Some(SubType::Savings),
        binding: Some(crate::model::store::Binding::Asset(usd())),
        ..StoreExtras::default()
    };
    let savings = vault
        .create_store_with("Reserve", StoreKind::Bank, "#000", "", Some(bank), extras)
        .unwrap()
        .id;
    vault
        .set_emergency_fund(EmergencyFund {
            scope: StoreScope {
                store_ids: vec![savings],
                include_children: false,
            },
            ..EmergencyFund::default()
        })
        .unwrap();
    let july = secs_or_now("2026-07-03T10:00:00Z");
    let sent = vault
        .health_notice(july, "UTC", false)
        .unwrap()
        .expect("sent");
    assert!(sent.body.contains("red"), "{}", sent.body);
}

// -- pins for the methods no test named, and the figures on a bare document --

#[test]
fn savings_series_lists_twelve_finished_months_oldest_first() {
    let (_dir, vault) = empty_vault();
    let now = secs_or_now("2026-06-15T10:00:00Z");
    let series = vault.savings_series(now, "UTC").unwrap();
    assert_eq!(series.len(), SAVINGS_MONTHS);
    assert_eq!(
        series.first().map(|row| row.month.as_str()),
        Some("2025-06")
    );
    assert_eq!(series.last().map(|row| row.month.as_str()), Some("2026-05"));
    assert!(series.iter().all(|row| row.saved_pct.is_none()));
}

#[test]
fn the_freedom_rate_is_saved_cleared_and_never_negative() {
    let (_dir, mut vault) = empty_vault();
    vault.set_freedom_rate(Some(pct("7.5"))).unwrap();
    assert_eq!(vault.doc.settings.freedom_rate, Some(pct("7.5")));
    assert!(vault.set_freedom_rate(Some(pct("-1"))).is_err());
    assert_eq!(vault.doc.settings.freedom_rate, Some(pct("7.5")));
    vault.set_freedom_rate(None).unwrap();
    assert_eq!(vault.doc.settings.freedom_rate, None);
}

#[test]
fn savings_figures_average_the_last_months_with_income() {
    let month = |name: &str, income: &str, saved: &str| SavingsMonth {
        month: name.into(),
        income: pct(income),
        saved: pct(saved),
        saved_pct: None,
    };
    let series = vec![
        month("2026-01", "0", "0"),
        month("2026-02", "1000", "100"),
        month("2026-03", "1000", "300"),
    ];
    let (avg, gap) = savings_figures(&series, pct("0.2"));
    assert_eq!(avg, Some(pct("20")));
    assert_eq!(gap, Some(pct("0")));
    assert_eq!(savings_figures(&[], pct("0.2")), (None, None));
}

#[test]
fn a_negative_freedom_rate_is_refused() {
    use crate::planning::health::check_freedom_rate;
    assert!(check_freedom_rate(None).is_ok());
    assert!(check_freedom_rate(Some(Amount::ZERO)).is_ok());
    assert!(check_freedom_rate(Some(pct("7.5"))).is_ok());
    assert!(check_freedom_rate(Some(pct("-1"))).is_err());
}

#[test]
fn the_average_of_months_needs_flow_in_every_one() {
    use crate::planning::health::months_average;
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let mut salary = income(&card, "1000", "salary");
    at(&mut salary, "2026-01-10T00:00:00Z");
    vault.add_transaction(salary).unwrap();
    let january = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let february = chrono::NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    let books = vault.books();
    let (income_avg, spent) = months_average(&books, &[january], "UTC").unwrap();
    assert_eq!((income_avg, spent), (pct("1000"), Amount::ZERO));
    assert_eq!(months_average(&books, &[january, february], "UTC"), None);
    assert_eq!(months_average(&books, &[], "UTC"), None);
}

#[test]
fn the_health_delegates_equal_the_scenarios() {
    use crate::usecases::planning::health as case;
    let (_a, mut by_method) = empty_vault();
    let (_b, mut by_case) = empty_vault();
    by_method.set_freedom_rate(Some(pct("7.5"))).unwrap();
    case::set_freedom_rate(&mut by_case, Some(pct("7.5"))).unwrap();
    assert_eq!(
        by_method.doc.settings.freedom_rate,
        by_case.doc.settings.freedom_rate
    );
    let now = secs_or_now("2026-04-02T10:00:00Z");
    let first = by_method.health_notice(now, "UTC", true).unwrap();
    let second = case::health_notice(&mut by_case, now, "UTC", true).unwrap();
    assert_eq!(first, second);
    assert_eq!(by_method.doc.health_last, by_case.doc.health_last);
}
