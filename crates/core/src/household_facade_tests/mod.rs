//! Tests of `household/` items that need the vault (`Session` and its fixtures). They moved
//! here from the `household` crate with their assertions unchanged so `household/` holds pure tests only
//! (crate-split T15). The files are named `<stem>_tests.rs`. The scenario support of the shared events tests
//! stays in `service/shared_events/testing.rs`, where the strangler map listed it.

mod analytics_tests;
mod attachments_cleanup_tests;
mod attachments_tests;
mod contacts_tests;
mod debts_tests;
mod ledger_tests;
mod losses_tests;
mod loyalty_tests;
mod property_links_tests;
mod purchases_tests;
mod relations_tests;
mod root_tests;
mod settle_tests;
// The scenarios the shared events tests share.
#[path = "../service/shared_events/testing.rs"]
mod testing;
mod text_tests;
mod utilities_tests;
mod vehicle_tests;
