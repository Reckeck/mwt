//! The `household` module group (spec core-strangler, D1).

pub mod attachments;
pub mod attachments_cleanup;
#[cfg(test)]
mod attachments_cleanup_tests;
#[cfg(test)]
mod attachments_tests;
pub mod contacts;
#[cfg(test)]
mod contacts_tests;
pub mod debts;
#[cfg(test)]
mod debts_tests;
pub mod losses;
#[cfg(test)]
mod losses_tests;
pub mod loyalty;
#[cfg(test)]
mod loyalty_tests;
pub mod property_links;
#[cfg(test)]
mod property_links_tests;
pub mod relations;
#[cfg(test)]
mod relations_tests;
pub mod shared_events;
#[cfg(test)]
mod shared_events_tests;
pub mod utilities;
#[cfg(test)]
mod utilities_tests;
pub mod vehicle;
#[cfg(test)]
mod vehicle_tests;
