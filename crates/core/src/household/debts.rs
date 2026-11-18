//! Debt load (plan C2): every loan and credit card with its class against the
//! key rate and inflation, and the debt-to-income ratio (DTI).
//!
//! Pure part of the area: the status of one loan and the totals over a
//! document and a read context. The orchestration that asks the not-yet-moved
//! areas (budget currency, inflation, key rate, income) is the read scenario in
//! `usecases::household::debts`.

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::kernel::books::Books;
use crate::model::VaultDoc;
use crate::model::store::{LiabilityKind, Store};
use crate::pricing::history::secs_or_now;

/// Points over inflation a rate may reach before it counts as costly, for a
/// currency without a key rate (plan C2).
const NO_KEY_RATE_MARGIN: u32 = 5;
/// Finished months whose repayments stand in for a missing monthly payment,
/// and whose income DTI is measured against.
pub const DTI_MONTHS: u32 = 3;
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

/// What one loan's status is computed against, the same for every loan.
pub struct LoanContext<'ctx> {
    /// The budget currency every amount is in.
    pub currency: &'ctx str,
    pub now: i64,
    /// `[from, to)` of the finished months whose repayments stand in for a
    /// missing monthly payment.
    pub repaid_window: (i64, i64),
    pub key_rate: Option<Amount>,
}

/// What one loan needs from the areas that have not moved yet.
pub struct LoanInputs {
    /// The currency the loan is kept in.
    pub own_currency: String,
    /// The loan's value in USD (negative when something is owed).
    pub balance_usd: Amount,
    /// Inflation of the loan's currency in the current month.
    pub inflation: Option<Amount>,
}

/// The live debt stores: loans and credit cards that are not archived.
pub fn loan_stores(doc: &VaultDoc) -> impl Iterator<Item = &Store> {
    doc.stores
        .iter()
        .filter(|store| store.is_debt() && !store.archived)
}

/// The status of one loan; `None` when nothing is owed and no payment is due.
pub fn loan_status(
    books: &Books<'_>,
    store: &Store,
    context: &LoanContext<'_>,
    inputs: LoanInputs,
) -> Option<LoanStatus> {
    let LoanContext {
        currency,
        now,
        repaid_window: (from, to),
        key_rate,
    } = *context;
    let LoanInputs {
        own_currency,
        balance_usd,
        inflation,
    } = inputs;
    let book = books.rate_book();
    let terms = store.liability.as_ref();
    let kind = terms.map_or(LiabilityKind::Other, |terms| terms.kind);
    let owed = book
        .convert(-balance_usd.min(Amount::ZERO), "usd", currency, now)
        .unwrap_or(Amount::ZERO)
        .round(2);
    let (payment, payment_from_fact) = match terms.and_then(|terms| terms.monthly_payment) {
        Some(payment) => (
            book.convert(payment, &own_currency, currency, now)
                .unwrap_or(payment)
                .round(2),
            false,
        ),
        None => (
            repaid_into(books, store, currency, from, to) / Amount::from(DTI_MONTHS),
            true,
        ),
    };
    if !owed.is_positive() && !payment.is_positive() {
        return None;
    }
    let rate = terms.and_then(|terms| terms.rate);
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
}

/// The income DTI is measured against: its monthly average and the finished
/// months behind it (0 for the hand-entered month).
pub struct IncomeBasis {
    pub income: Amount,
    pub months: u32,
}

/// The totals over the loans and the DTI against the income.
pub fn debt_load_of(
    currency: String,
    loans: Vec<LoanStatus>,
    basis: Option<IncomeBasis>,
    key_rate: Option<Amount>,
) -> DebtLoad {
    let owed = loans.iter().fold(Amount::ZERO, |sum, loan| sum + loan.owed);
    let payments = loans
        .iter()
        .fold(Amount::ZERO, |sum, loan| sum + loan.payment);
    let income = basis.as_ref().map(|avg| avg.income.round(2));
    let dti_pct = income
        .filter(|value| value.is_positive())
        .map(|value| (payments / value * Amount::from(100u32)).round(1));
    DebtLoad {
        currency,
        loans,
        owed,
        payments,
        income,
        income_months: basis.as_ref().map_or(0, |avg| avg.months),
        dti_pct,
        key_rate,
    }
}

/// `[from, to)` of the last `months` finished calendar months before `now`.
pub fn finished_months(cal: &Calendar, now: i64, months: u32) -> (i64, i64) {
    let first = calendar::first_of_month(cal.date_of(now));
    let mut start = first;
    for _ in 0..months {
        start = calendar::first_of_month(start - chrono::Duration::days(1));
    }
    (cal.start_of(start), cal.start_of(first))
}

/// What came into a debt store in `[from, to)` (its repayments) in `currency`.
fn repaid_into(books: &Books<'_>, store: &Store, currency: &str, from: i64, to: i64) -> Amount {
    let book = books.rate_book();
    books
        .doc()
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
