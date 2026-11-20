//! Scenarios of the emergency fund: each decides in `planning::emergency`,
//! then ends in one `vault.transact`. The status gathers what other areas
//! know (the income basis of the budget, the interest booked) and leaves the
//! rules to `planning::emergency`.

use crate::Amount;
use crate::error::Result;
use crate::kernel::books::Books;
use crate::model::planning::EmergencyFund;
use crate::planning::budget::income_basis;
use crate::planning::emergency::{
    EmergencyStatus, fund_status, plan_set_emergency_fund, prune_emergency_fund as prune,
    toggled_fund,
};
use crate::pricing::history::secs_or_now;
use crate::service::UnlockedVault;
use crate::service::interest::INTEREST_CATEGORY;

const YEAR_SECS: i64 = 365 * 86_400;

/// Replace the fund. Only live savings accounts fit (decision 29); an
/// empty list removes the fund.
pub fn set_emergency_fund(vault: &mut UnlockedVault, fund: EmergencyFund) -> Result<EmergencyFund> {
    let next = plan_set_emergency_fund(&vault.doc, fund)?;
    vault.transact(|tx| {
        tx.doc_mut().emergency_fund = next.clone();
        Ok(next)
    })
}

/// Drop accounts that stopped being live savings (decision 36). The caller
/// saves: this runs inside the writes that archive or re-purpose a store.
pub(crate) fn prune_emergency_fund(vault: &mut UnlockedVault) {
    prune(&mut vault.doc);
}

/// Add or remove one account, the switch in the store form.
pub fn emergency_fund_toggle(
    vault: &mut UnlockedVault,
    store_id: &str,
    on: bool,
) -> Result<EmergencyFund> {
    let fund = toggled_fund(&vault.doc, store_id, on);
    set_emergency_fund(vault, fund)
}

/// The fund's value, coverage and hints.
pub fn emergency_fund_status(books: &Books<'_>, now: i64, zone: &str) -> EmergencyStatus {
    let basis = income_basis(
        books,
        books.doc().emergency_fund.avg_months as usize,
        now,
        zone,
    );
    fund_status(
        books,
        basis,
        |store_id, currency| interest_since(books, store_id, currency, now - YEAR_SECS, now),
        now,
        zone,
    )
}

/// Interest booked into a store in `[from, to)`, in `currency`.
fn interest_since(books: &Books<'_>, store_id: &str, currency: &str, from: i64, to: i64) -> Amount {
    let book = books.rate_book();
    books
        .doc()
        .transactions
        .iter()
        .filter(|tx| {
            tx.meta()
                .categories
                .iter()
                .any(|key| key == INTEREST_CATEGORY)
        })
        .filter_map(|tx| {
            let secs = secs_or_now(&tx.meta().timestamp);
            (secs >= from && secs < to).then_some((tx, secs))
        })
        .flat_map(|(tx, secs)| tx.legs().into_iter().map(move |leg| (leg, secs)))
        .filter(|(leg, _)| leg.store_id == store_id && leg.amount.is_positive())
        .filter_map(|(leg, secs)| book.convert(leg.amount, &leg.asset.symbol, currency, secs))
        .fold(Amount::ZERO, |sum, value| sum + value)
        .round(2)
}
