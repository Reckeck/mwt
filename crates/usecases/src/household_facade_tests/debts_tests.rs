//! Tests of the debts area: the old inline tests (names and bodies unchanged).

use crate::Amount;
use crate::household::debts::*;
use crate::model::store::{LiabilityKind, LiabilityTerms, StoreKind};
use crate::model::transaction::AccountRef;
use crate::planning::debt_load::debt_load;
use crate::service::test_support::*;
use crate::service::{Session, StoreExtras};

fn pct(raw: &str) -> Option<Amount> {
    Some(amount(raw))
}

#[test]
fn a_loan_is_classed_by_its_rate() {
    let owed = amount("100");
    let loan = LiabilityKind::Loan;
    let class = |rate| classify_loan(loan, owed, rate, pct("16"), pct("6"));
    assert_eq!(class(pct("21")), LoanClass::Costly);
    assert_eq!(class(pct("16")), LoanClass::Normal);
    assert_eq!(class(pct("6")), LoanClass::Normal);
    assert_eq!(class(pct("4")), LoanClass::Cheap);
    assert_eq!(class(None), LoanClass::Unknown);
    // nothing to compare with: a known rate is just normal
    assert_eq!(
        classify_loan(loan, owed, pct("30"), None, None),
        LoanClass::Normal
    );
}

#[test]
fn a_card_with_a_debt_is_costly_whatever_its_rate() {
    let card = LiabilityKind::CreditCard;
    assert_eq!(
        classify_loan(card, amount("10"), pct("0"), pct("16"), pct("6")),
        LoanClass::Costly
    );
    assert_eq!(
        classify_loan(card, Amount::ZERO, None, pct("16"), pct("6")),
        LoanClass::Unknown
    );
}

fn loan(vault: &mut Session, name: &str, terms: LiabilityTerms) -> String {
    crate::usecases::money::stores::create_store_with(
        vault,
        name,
        StoreKind::Liability,
        "",
        "",
        None,
        StoreExtras {
            liability: Some(terms),
            ..StoreExtras::default()
        },
    )
    .unwrap()
    .id
}

fn terms(kind: LiabilityKind, rate: Option<&str>, payment: Option<&str>) -> LiabilityTerms {
    LiabilityTerms {
        kind,
        rate: rate.map(amount),
        limit: None,
        due_day: None,
        monthly_payment: payment.map(amount),
    }
}

fn dated(
    mut tx: crate::model::transaction::Transaction,
    stamp: &str,
) -> crate::model::transaction::Transaction {
    at(&mut tx, stamp);
    tx
}

#[test]
fn dti_takes_terms_or_recent_repayments_against_income() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    let mortgage = loan(
        &mut vault,
        "Mortgage",
        terms(LiabilityKind::Mortgage, Some("2"), Some("300")),
    );
    let card = loan(
        &mut vault,
        "Card",
        terms(LiabilityKind::CreditCard, None, None),
    );
    // a paid-off loan with no payment is not a burden
    loan(
        &mut vault,
        "Old",
        terms(LiabilityKind::Loan, Some("9"), None),
    );
    let draw = |to: &str, raw: &str, stamp: &str| {
        dated(
            transfer_between(
                AccountRef::Store(to.to_string()),
                AccountRef::Store(bank.clone()),
                usd(),
                raw,
            ),
            stamp,
        )
    };
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        draw(&mortgage, "10000", "2026-05-02T10:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        draw(&card, "900", "2026-05-02T10:00:00Z"),
    )
    .unwrap();
    for month in ["06", "07", "08"] {
        let mut salary = income(&bank, "2000", "salary");
        salary.meta_mut().categories = vec!["salary".into()];
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            dated(salary, &format!("2026-{month}-05T10:00:00Z")),
        )
        .unwrap();
        let repay = transfer_between(
            AccountRef::Store(bank.clone()),
            AccountRef::Store(card.clone()),
            usd(),
            "100",
        );
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            dated(repay, &format!("2026-{month}-20T10:00:00Z")),
        )
        .unwrap();
    }

    let now = unix("2026-09-10T10:00:00Z");
    let load = debt_load(&vault.books(), now, "UTC").unwrap();
    assert_eq!(load.loans.len(), 2);
    let find = |id: &str| load.loans.iter().find(|loan| loan.store_id == id).unwrap();
    let home = find(&mortgage);
    assert_eq!(home.class, LoanClass::Cheap);
    assert_eq!(home.benchmark, pct("8"));
    assert_eq!(home.payment, amount("300"));
    assert!(!home.payment_from_fact);
    let plastic = find(&card);
    assert_eq!(plastic.class, LoanClass::Costly);
    assert_eq!(plastic.owed, amount("600"));
    assert_eq!(plastic.payment, amount("100"));
    assert!(plastic.payment_from_fact);
    assert_eq!(load.payments, amount("400"));
    assert_eq!(load.income, Some(amount("2000")));
    assert_eq!(load.dti_pct, pct("20"));
}

fn unix(stamp: &str) -> i64 {
    crate::time::unix_secs(stamp).unwrap()
}

#[test]
fn the_load_totals_and_the_dti_need_a_positive_income() {
    let loans = Vec::new();
    let basis = |income: &str| {
        Some(IncomeBasis {
            income: amount(income),
            months: 3,
        })
    };
    let load = debt_load_of("usd".into(), loans.clone(), basis("2000"), None);
    assert_eq!(load.income, Some(amount("2000")));
    assert_eq!(load.income_months, 3);
    assert_eq!(load.dti_pct, Some(Amount::ZERO));
    assert_eq!(
        debt_load_of("usd".into(), loans, basis("0"), None).dti_pct,
        None
    );
}
