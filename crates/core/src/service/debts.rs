//! Debt load (plan C2): every loan and credit card with its class against the
//! key rate and inflation, and the debt-to-income ratio (DTI).

use super::UnlockedVault;
use crate::calendar::{self, Calendar};
use crate::model::store::{LiabilityKind, Store};
use crate::pricing::history::secs_or_now;
use crate::{Amount, Result};

/// Points over inflation a rate may reach before it counts as costly, for a
/// currency without a key rate (plan C2).
const NO_KEY_RATE_MARGIN: u32 = 5;
/// Finished months whose repayments stand in for a missing monthly payment,
/// and whose income DTI is measured against.
const DTI_MONTHS: u32 = 3;
/// The one currency the key rate series is about.
const KEY_RATE_CURRENCY: &str = "rub";

/// How a loan's rate compares with money's price (decision 16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum LoanClass {
    /// Above the key rate, or a credit card carrying a debt: pay it off first.
    Costly,
    Normal,
    /// Below inflation: inflation repays part of it.
    Cheap,
    /// No rate entered.
    Unknown,
}

/// One loan or credit card that owes money or has a payment.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct LoanStatus {
    pub store_id: String,
    pub kind: LiabilityKind,
    /// The currency the loan is kept in.
    pub own_currency: String,
    /// What is owed, positive, in the budget currency.
    pub owed: Amount,
    /// Annual rate, percent.
    pub rate: Option<Amount>,
    /// What the rate was compared with: the key rate, or inflation plus a
    /// margin; `None` when neither is known.
    pub benchmark: Option<Amount>,
    pub inflation: Option<Amount>,
    pub class: LoanClass,
    /// Monthly payment in the budget currency.
    pub payment: Amount,
    /// The payment is the average repayment of recent months, not the terms.
    pub payment_from_fact: bool,
}

/// Loans and the share of income they take (plan C2).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct DebtLoad {
    /// The budget currency every amount here is in.
    pub currency: String,
    pub loans: Vec<LoanStatus>,
    pub owed: Amount,
    pub payments: Amount,
    /// Average monthly income; `None` without history or a hand-entered month.
    pub income: Option<Amount>,
    /// Finished months behind `income`; 0 when it is the hand-entered month.
    pub income_months: u32,
    /// `payments / income`, percent.
    pub dti_pct: Option<Amount>,
    pub key_rate: Option<Amount>,
}

/// The class of a loan: a card with a debt is costly whatever its rate,
/// otherwise the rate decides against `benchmark` and `inflation`.
pub fn classify_loan(
    kind: LiabilityKind,
    owed: Amount,
    rate: Option<Amount>,
    benchmark: Option<Amount>,
    inflation: Option<Amount>,
) -> LoanClass {
    if kind == LiabilityKind::CreditCard && owed.is_positive() {
        return LoanClass::Costly;
    }
    let Some(rate) = rate else {
        return LoanClass::Unknown;
    };
    if benchmark.is_some_and(|line| rate > line) {
        LoanClass::Costly
    } else if inflation.is_some_and(|line| rate < line) {
        LoanClass::Cheap
    } else {
        LoanClass::Normal
    }
}

impl UnlockedVault {
    /// Every live loan and credit card with a debt or a payment, their class
    /// and DTI. `manual` is the hand-entered inflation table.
    pub fn debt_load(&self, now: i64, zone: &str) -> Result<DebtLoad> {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let month = calendar::month_key(today);
        let currency = self.budget_currency();
        let book = self.rate_book();
        let values = self.store_values_usd();
        let key_rate = self.key_rate_on(today);
        let (from, to) = self.finished_months(&cal, now, DTI_MONTHS);

        let loans: Vec<LoanStatus> = self
            .doc
            .stores
            .iter()
            .filter(|store| store.is_debt() && !store.archived)
            .filter_map(|store| {
                let terms = store.liability.as_ref();
                let kind = terms.map_or(LiabilityKind::Other, |terms| terms.kind);
                let own_currency = self.store_currency(&store.id);
                let balance = values.get(&store.id).copied().unwrap_or_default();
                let owed = book
                    .convert(-balance.min(Amount::ZERO), "usd", &currency, now)
                    .unwrap_or(Amount::ZERO)
                    .round(2);
                let (payment, payment_from_fact) =
                    match terms.and_then(|terms| terms.monthly_payment) {
                        Some(payment) => (
                            book.convert(payment, &own_currency, &currency, now)
                                .unwrap_or(payment)
                                .round(2),
                            false,
                        ),
                        None => (
                            self.repaid_into(store, &currency, from, to) / Amount::from(DTI_MONTHS),
                            true,
                        ),
                    };
                if !owed.is_positive() && !payment.is_positive() {
                    return None;
                }
                let rate = terms.and_then(|terms| terms.rate);
                let inflation = self.inflation_on(&own_currency, &month);
                let benchmark = if own_currency == KEY_RATE_CURRENCY && key_rate.is_some() {
                    key_rate
                } else {
                    inflation.map(|value| value + Amount::from(NO_KEY_RATE_MARGIN))
                };
                Some(LoanStatus {
                    store_id: store.id.clone(),
                    kind,
                    own_currency,
                    owed,
                    rate,
                    benchmark,
                    inflation,
                    class: classify_loan(kind, owed, rate, benchmark, inflation),
                    payment: payment.round(2),
                    payment_from_fact: payment_from_fact && payment.is_positive(),
                })
            })
            .collect();

        let owed = loans.iter().fold(Amount::ZERO, |sum, loan| sum + loan.owed);
        let payments = loans
            .iter()
            .fold(Amount::ZERO, |sum, loan| sum + loan.payment);
        let basis = self.income_basis(DTI_MONTHS as usize, now, zone);
        let income = basis.as_ref().map(|avg| avg.income.round(2));
        let dti_pct = income
            .filter(|value| value.is_positive())
            .map(|value| (payments / value * Amount::from(100u32)).round(1));
        Ok(DebtLoad {
            currency,
            loans,
            owed,
            payments,
            income,
            income_months: basis.as_ref().map_or(0, |avg| avg.months),
            dti_pct,
            key_rate,
        })
    }

    /// `[from, to)` of the last `months` finished calendar months before `now`.
    fn finished_months(&self, cal: &Calendar, now: i64, months: u32) -> (i64, i64) {
        let first = calendar::first_of_month(cal.date_of(now));
        let mut start = first;
        for _ in 0..months {
            start = calendar::first_of_month(start - chrono::Duration::days(1));
        }
        (cal.start_of(start), cal.start_of(first))
    }

    /// What came into a debt store in `[from, to)` — its repayments — in
    /// `currency`.
    fn repaid_into(&self, store: &Store, currency: &str, from: i64, to: i64) -> Amount {
        let book = self.rate_book();
        self.doc
            .transactions
            .iter()
            .filter_map(|tx| {
                let secs = secs_or_now(&tx.meta().timestamp);
                (secs >= from && secs < to).then_some((tx, secs))
            })
            .flat_map(|(tx, secs)| tx.legs().into_iter().map(move |leg| (leg, secs)))
            .filter(|(leg, _)| leg.store_id == store.id && leg.amount.is_positive())
            .filter_map(|(leg, secs)| book.convert(leg.amount, &leg.asset.symbol, currency, secs))
            .fold(Amount::ZERO, |sum, value| sum + value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::{LiabilityTerms, StoreKind};
    use crate::model::transaction::AccountRef;
    use crate::service::StoreExtras;
    use crate::service::test_support::*;

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

    fn loan(vault: &mut UnlockedVault, name: &str, terms: LiabilityTerms) -> String {
        vault
            .create_store_with(
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
        vault
            .add_transaction(draw(&mortgage, "10000", "2026-05-02T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(draw(&card, "900", "2026-05-02T10:00:00Z"))
            .unwrap();
        for month in ["06", "07", "08"] {
            let mut salary = income(&bank, "2000", "salary");
            salary.meta_mut().categories = vec!["salary".into()];
            vault
                .add_transaction(dated(salary, &format!("2026-{month}-05T10:00:00Z")))
                .unwrap();
            let repay = transfer_between(
                AccountRef::Store(bank.clone()),
                AccountRef::Store(card.clone()),
                usd(),
                "100",
            );
            vault
                .add_transaction(dated(repay, &format!("2026-{month}-20T10:00:00Z")))
                .unwrap();
        }

        let now = unix("2026-09-10T10:00:00Z");
        let load = vault.debt_load(now, "UTC").unwrap();
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
        crate::pricing::history::unix_secs(stamp).unwrap()
    }
}
