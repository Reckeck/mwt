//! The crypto module's engine: where operations come from and what the
//! module refuses to store (stage 2, spec E0).
//!
//! What is *stored* lives in `mwt_model`'s crypto module; this module holds
//! the traits a source implements and the input rules every way into the
//! module goes through.

#![warn(unreachable_pub)]

pub mod chains;
pub mod engine;
pub mod exchange_import;
pub mod exchange_sync;
#[cfg(test)]
mod exchange_sync_tests;
pub mod ports;
pub mod prices;
pub mod protocols;
pub mod service;
pub mod source;
pub mod sources;
#[cfg(test)]
pub(crate) mod test_chain;
pub mod tokens;
pub mod tracking;
#[cfg(test)]
mod tracking_tests;
pub mod validate;

pub use protocols::{ProtocolAdapter, ProtocolRequest, Registry as ProtocolRegistry};
pub use source::{FetchPage, FetchRequest, Source};
pub use validate::validate_section;
