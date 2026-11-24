//! Pure planning rules (no vault, no I/O).
#![warn(unreachable_pub)]

pub mod agenda;
pub mod allocation;
#[cfg(test)]
mod allocation_props;
pub mod auto_reminders;
pub mod automation_card;
pub mod automation_plan;
pub mod automations;
pub mod budget;
#[cfg(test)]
mod budget_props;
pub mod daily_budget;
pub mod datasources;
pub mod debt_load;
pub mod emergency;
pub mod envelopes;
pub mod flows;
#[cfg(test)]
mod flows_basis_tests;
pub mod goals;
pub mod health;
pub mod my_automations;
pub mod notifications;
pub mod passive;
pub mod period;
#[cfg(test)]
mod period_props;
pub mod recurring;
pub mod scenarios;
pub mod schedule;
pub mod upcoming;
