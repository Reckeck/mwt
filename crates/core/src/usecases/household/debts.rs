//! Read scenario of the debts area: asks the areas that have not moved yet
//! (budget currency, inflation, key rate, income) and hands the answers to the
//! pure functions of `household::debts`.

use crate::calendar::{self, Calendar};
use crate::error::Result;
use crate::household::debts::{
    DTI_MONTHS, DebtLoad, IncomeBasis, LoanContext, LoanInputs, debt_load_of, finished_months,
    loan_status, loan_stores,
};
use crate::planning::budget::income_basis;
use crate::service::UnlockedVault;

/// Every live loan and credit card with a debt or a payment, their class
/// and DTI. `manual` is the hand-entered inflation table.
pub fn debt_load(vault: &UnlockedVault, now: i64, zone: &str) -> Result<DebtLoad> {
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let month = calendar::month_key(today);
    let currency = vault.budget_currency();
    let values = vault.store_values_usd();
    let key_rate = vault.key_rate_on(today);
    let books = vault.books();
    let context = LoanContext {
        currency: &currency,
        now,
        repaid_window: finished_months(&cal, now, DTI_MONTHS),
        key_rate,
    };
    let loans = loan_stores(&vault.doc)
        .filter_map(|store| {
            let own_currency = vault.store_currency(&store.id);
            let inputs = LoanInputs {
                balance_usd: values.get(&store.id).copied().unwrap_or_default(),
                inflation: vault.inflation_on(&own_currency, &month),
                own_currency,
            };
            loan_status(&books, store, &context, inputs)
        })
        .collect();
    let basis = income_basis(&books, DTI_MONTHS as usize, now, zone).map(|avg| IncomeBasis {
        income: avg.income,
        months: avg.months,
    });
    Ok(debt_load_of(currency, loans, basis, key_rate))
}
