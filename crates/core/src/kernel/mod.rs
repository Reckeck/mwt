//! The kernel of the core: the one place for how a change of the open vault
//! is committed. It grows by the strangler rule; the first step only pins the
//! write path as it is, so the later moves can be checked against it.

pub mod books;
pub mod collection;
pub mod events;
#[cfg(test)]
mod golden_tests;
pub mod persistence;
pub mod record;
pub mod session;
pub mod tx;
