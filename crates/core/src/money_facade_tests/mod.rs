//! Tests of `money/` items that need the vault (`UnlockedVault` and its fixtures). They moved
//! here from `money/` with their assertions unchanged so `money/` holds pure tests only (crate-split T13).

mod adjustments;
mod asset_holdings_tests;
mod bulk;
mod bulk_tests;
mod categories;
mod cost_basis;
mod cost_basis_tests;
mod enrichment;
mod inflation;
mod inflation_tests;
mod interest;
mod investments_tests;
mod ledger;
mod month_balance;
mod portfolio_tests;
mod preview;
mod preview_tests;
mod price_info;
mod rates;
mod rebase_tests;
mod search_tests;
mod store_change;
mod store_months;
mod stores;
mod transactions;
mod validate;
mod valuation;
mod valuation_tests;
mod worth;
mod worth_tests;
