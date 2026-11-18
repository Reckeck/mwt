//! Scenarios of the store_months area: a read, so no write. It hands the budget
//! area's valuation of a leg to the pure function (OD6) until that area moves.

use crate::model::ids::StoreId;
use crate::money::store_months::{StoreMonthRow, store_months as monthly_rows};
use crate::service::UnlockedVault;

/// The last `months` local months up to the one holding `now`, oldest first.
/// Moves between two stores of the scope are left out: they are neither.
pub fn store_months(
    vault: &UnlockedVault,
    store_id: &StoreId,
    include_children: bool,
    months: u32,
    now: i64,
    zone: &str,
) -> Vec<StoreMonthRow> {
    let books = vault.books();
    monthly_rows(
        &books,
        store_id,
        include_children,
        months,
        now,
        zone,
        |leg, currency, secs| UnlockedVault::leg_value_in(books.rate_book(), leg, currency, secs),
    )
}
