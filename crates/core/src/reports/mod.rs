//! The `reports` module group (spec core-strangler, D1).

pub(crate) mod crypto_module;
pub mod dashboard;
#[cfg(test)]
mod dashboard_tests;
pub mod dashboard_tiles;
#[cfg(test)]
mod dashboard_tiles_tests;
pub mod dashboard_today;
#[cfg(test)]
mod dashboard_today_tests;
pub mod health;
pub mod insights;
#[cfg(test)]
mod insights_tests;
pub mod notify_snapshot;
#[cfg(test)]
mod notify_snapshot_tests;
pub mod overview;
#[cfg(test)]
mod overview_tests;
pub mod where_went;
#[cfg(test)]
mod where_went_tests;
