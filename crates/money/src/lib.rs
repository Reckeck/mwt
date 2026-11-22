//! Pure money rules: no vault, clock or I/O.

pub mod adjustments;
pub mod asset_holdings;
pub mod books;
pub mod bulk;
pub mod categories;
#[cfg(test)]
mod categories_tests;
pub mod cost_basis;
pub mod date_label;
pub mod enrichment;
pub mod inflation;
#[cfg(test)]
mod inflation_props;
pub mod interest;
#[cfg(test)]
mod interest_props;
#[cfg(test)]
mod interest_tests;
pub mod investments;
pub mod ledger;
pub mod month_balance;
#[cfg(test)]
mod month_balance_tests;
pub mod portfolio;
pub mod preview;
pub mod price_info;
pub mod rate_index;
pub mod rates;
pub mod rebase;
pub mod search;
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
pub mod worth;
