//! Tests of `planning/` items that need the vault (`Session` and its fixtures). They moved here
//! from `planning/` with their assertions unchanged so that module holds pure tests only
//! (crate-split T17). The files are named `<stem>_tests.rs`.

mod agenda_tests;
// Stays under `service/agenda/`: the strangler map listed that directory in the agenda area.
#[path = "../service/agenda/alert_tests.rs"]
mod alert_tests;
mod allocation_tests;
mod auto_reminders_tests;
mod automation_card_tests;
mod automations_tests;
mod budget_tests;
mod datasources_tests;
mod emergency_tests;
mod envelopes_tests;
mod events_tests;
mod goals_tests;
mod health_tests;
mod my_automations_tests;
mod notifications_tests;
mod passive_tests;
mod recurring_tests;
mod scenarios_tests;
