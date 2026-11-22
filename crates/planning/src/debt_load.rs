//! The debt load read (plan C2): asks the budget (currency, income), the
//! goals (store values), inflation and the key rate, and hands the answers to
//! the pure functions of `household::debts`. It sits in `planning` because the
//! income basis does; `household` may not call up.

use crate::budget::{budget_currency, income_basis, store_currency};
use crate::goals::store_values_usd;
use mwt_base::calendar::{self, Calendar};
use mwt_base::error::Result;
use mwt_household::debts::{
    DTI_MONTHS, DebtLoad, IncomeBasis, LoanContext, LoanInputs, debt_load_of, finished_months,
    loan_status, loan_stores,
};
use mwt_money::books::Books;
use mwt_money::inflation::inflation_on;
use mwt_money::interest::key_rate_on;

/// Every live loan and credit card with a debt or a payment, their class
/// and DTI. `manual` is the hand-entered inflation table.
pub fn debt_load(books: &Books<'_>, now: i64, zone: &str) -> Result<DebtLoad> {
    let doc = books.doc();
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let month = calendar::month_key(today);
    let currency = budget_currency(books);
    let values = store_values_usd(books);
    let key_rate = key_rate_on(doc, today);
    let context = LoanContext {
        currency: &currency,
        now,
        repaid_window: finished_months(&cal, now, DTI_MONTHS),
        key_rate,
    };
    let loans = loan_stores(doc)
        .filter_map(|store| {
            let own_currency = store_currency(books, &store.id);
            let inputs = LoanInputs {
                balance_usd: values.get(&store.id).copied().unwrap_or_default(),
                inflation: inflation_on(doc, &own_currency, &month),
                own_currency,
            };
            loan_status(books, store, &context, inputs)
        })
        .collect();
    let basis = income_basis(books, DTI_MONTHS as usize, now, zone).map(|avg| IncomeBasis {
        income: avg.income,
        months: avg.months,
    });
    Ok(debt_load_of(currency, loans, basis, key_rate))
}
