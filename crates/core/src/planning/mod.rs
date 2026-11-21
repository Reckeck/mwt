//! Pure planning rules (no vault, no I/O).

pub mod agenda;
#[cfg(test)]
mod agenda_tests;
pub mod allocation;
#[cfg(test)]
mod allocation_props;
#[cfg(test)]
mod allocation_tests;
pub mod auto_reminders;
#[cfg(test)]
mod auto_reminders_tests;
pub mod automation_card;
#[cfg(test)]
mod automation_card_tests;
pub mod automation_plan;
pub mod automations;
#[cfg(test)]
mod automations_tests;
pub mod budget;
#[cfg(test)]
mod budget_props;
#[cfg(test)]
mod budget_tests;
pub mod daily_budget;
pub mod datasources;
#[cfg(test)]
mod datasources_tests;
pub mod debt_load;
pub mod emergency;
#[cfg(test)]
mod emergency_tests;
pub mod envelopes;
#[cfg(test)]
mod envelopes_tests;
pub(crate) mod flows;
pub mod goals;
#[cfg(test)]
mod goals_tests;
pub mod health;
#[cfg(test)]
mod health_tests;
pub mod my_automations;
#[cfg(test)]
mod my_automations_tests;
pub mod notifications;
#[cfg(test)]
mod notifications_tests;
pub mod passive;
#[cfg(test)]
mod passive_tests;
pub mod period;
#[cfg(test)]
mod period_props;
pub mod recurring;
#[cfg(test)]
mod recurring_tests;
pub mod scenarios;
#[cfg(test)]
mod scenarios_tests;
pub mod schedule;
pub mod upcoming;
