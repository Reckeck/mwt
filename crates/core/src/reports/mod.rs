//! The `reports` module group (spec core-strangler, D1).

pub mod comparison;
pub(crate) mod crypto_module;
pub mod dashboard;
pub mod dashboard_tiles;
pub mod dashboard_today;
pub mod health;
pub mod insights;
pub(crate) mod net_worth_now;
pub mod notify_snapshot;
pub mod overview;
pub mod where_went;

#[cfg(test)]
mod where_went_tests;
