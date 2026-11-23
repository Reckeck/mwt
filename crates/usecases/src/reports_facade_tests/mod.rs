//! Tests of `reports/` items that need the vault (`Session` and its fixtures). They moved here
//! from `reports/` with their assertions unchanged so that module holds pure tests only
//! (crate-split T17). The files are named `<stem>_tests.rs`.

mod dashboard_tests;
mod dashboard_tiles_tests;
mod dashboard_today_tests;
mod insights_tests;
mod notify_snapshot_tests;
mod overview_tests;
