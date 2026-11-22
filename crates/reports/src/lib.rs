//! Reports: the overview, dashboard, insights, health, comparison, where-went and notification snapshot rules
//! over the books (no vault, no I/O; crate-split T19).
#![warn(unreachable_pub)]

pub mod comparison;
pub(crate) mod crypto_module;
pub mod dashboard;
pub mod dashboard_tiles;
pub mod dashboard_today;
pub mod health;
pub mod insights;
pub mod net_worth_now;
pub mod notify_snapshot;
pub mod overview;
pub mod where_went;

#[cfg(test)]
mod where_went_tests;
