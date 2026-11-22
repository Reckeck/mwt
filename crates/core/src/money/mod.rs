//! Pure money rules: no vault, clock or I/O.

pub mod adjustments;
pub mod asset_holdings;
#[cfg(test)]
mod asset_holdings_tests;
pub mod books;
pub mod bulk;
#[cfg(test)]
mod bulk_tests;
pub mod categories;
#[cfg(test)]
mod categories_tests;
pub mod cost_basis;
pub mod date_label;
pub mod enrichment;
pub mod inflation;
#[cfg(test)]
mod inflation_props;
#[cfg(test)]
mod inflation_tests;
pub mod interest;
#[cfg(test)]
mod interest_props;
#[cfg(test)]
mod interest_tests;
pub mod investments;
#[cfg(test)]
mod investments_tests;
pub mod ledger;
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
pub mod rate_index;
pub mod rates;
pub mod rebase;
#[cfg(test)]
mod rebase_tests;
pub mod search;
#[cfg(test)]
mod search_tests;
pub mod shared_ratios;
pub mod shared_split;
pub mod store_change;
pub mod store_months;
pub mod stores;
#[cfg(test)]
mod stores_tests;
pub mod transactions;
pub mod tree;
pub mod txinfo;
#[cfg(test)]
mod txinfo_props;
pub mod validate;
pub mod valuation;
#[cfg(test)]
mod valuation_tests;
pub mod worth;
#[cfg(test)]
mod worth_tests;
