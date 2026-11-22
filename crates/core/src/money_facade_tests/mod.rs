//! Tests of `money/` items that need the vault (`UnlockedVault` and its fixtures). They moved
//! here from `money/` with their assertions unchanged so `money/` holds pure tests only (crate-split T13).

mod adjustments;
mod bulk;
mod categories;
mod cost_basis;
mod enrichment;
mod inflation;
mod interest;
mod ledger;
mod month_balance;
mod preview;
mod price_info;
mod rates;
mod store_change;
mod store_months;
mod stores;
mod transactions;
mod validate;
mod valuation;
mod worth;
