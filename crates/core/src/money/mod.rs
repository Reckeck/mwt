//! Pure money rules: no vault, clock or I/O.

pub mod adjustments;
#[cfg(test)]
mod adjustments_tests;
pub mod asset_holdings;
#[cfg(test)]
mod asset_holdings_tests;
pub mod bulk;
#[cfg(test)]
mod bulk_tests;
pub mod categories;
#[cfg(test)]
mod categories_tests;
pub mod cost_basis;
pub mod enrichment;
#[cfg(test)]
mod enrichment_tests;
pub mod inflation;
#[cfg(test)]
mod inflation_tests;
pub mod interest;
#[cfg(test)]
mod interest_tests;
pub mod investments;
#[cfg(test)]
mod investments_tests;
pub mod ledger;
#[cfg(test)]
mod ledger_tests;
pub mod month_balance;
#[cfg(test)]
mod month_balance_tests;
pub mod portfolio;
#[cfg(test)]
mod portfolio_tests;
pub mod preview;
#[cfg(test)]
mod preview_tests;
pub mod price_info;
#[cfg(test)]
mod price_info_tests;
pub mod rates;
#[cfg(test)]
mod rates_tests;
pub mod rebase;
#[cfg(test)]
mod rebase_tests;
pub mod search;
#[cfg(test)]
mod search_tests;
pub mod shared_ratios;
pub mod store_change;
#[cfg(test)]
mod store_change_tests;
pub mod store_months;
#[cfg(test)]
mod store_months_tests;
pub mod stores;
#[cfg(test)]
mod stores_tests;
pub mod transactions;
#[cfg(test)]
mod transactions_tests;
pub mod tree;
pub mod validate;
#[cfg(test)]
mod validate_tests;
pub mod valuation;
#[cfg(test)]
mod valuation_tests;
pub mod worth;
#[cfg(test)]
mod worth_tests;
