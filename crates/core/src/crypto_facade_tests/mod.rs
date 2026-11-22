//! Tests of `cryptotrack/` items that need the vault (`UnlockedVault` and its fixtures) or a
//! higher layer. They moved here from `cryptotrack/` with their assertions unchanged so that
//! module holds pure tests only (crate-split T20). The files are named `<stem>_tests.rs`.

mod exchange_import_tests;
mod exchange_sync_intake_tests;
mod ports_tests;
mod service_tests;
mod volume_tests;
mod wipe_tests;
