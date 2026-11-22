//! Household: attachments, contacts, debts, losses, loyalty, property links, relations, shared events,
//! utilities and vehicles (spec crate-split, `mwt-household`). Pure rules over the vault document; the
//! scenarios that write live in `trade-core` (`usecases::household`).
#![warn(unreachable_pub)]

pub mod attachments;
pub mod attachments_cleanup;
pub mod contacts;
pub mod debts;
pub mod losses;
pub mod loyalty;
pub mod property_links;
pub mod relations;
pub mod shared_events;
#[cfg(test)]
mod shared_events_tests;
pub mod utilities;
pub mod vehicle;
