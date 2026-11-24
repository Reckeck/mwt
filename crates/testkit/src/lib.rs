#![warn(unreachable_pub)]
//! `mwt-testkit`: test support that needs the model, the base and the vault only. It is a
//! dev-only crate: no crate lists it as a normal dependency (`mwt-usecases` reaches
//! it through its `testkit` feature and its dev-dependencies).
//!
//! The helpers that need `Session` or `service::test_support` stay in `mwt-usecases`'s
//! `testkit` module, which re-exports this crate.

pub mod builders;
mod memory;
pub mod props;

pub use memory::InMemoryPersistence;
/// The money-blockers' clock: the one from `mwt-base`, not a copy.
pub use mwt_base::time::FixedClock;
