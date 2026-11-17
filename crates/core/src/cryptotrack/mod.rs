//! The crypto module's engine: where operations come from and what the
//! module refuses to store (stage 2, spec E0).
//!
//! What is *stored* lives in [`crate::model::cryptotrack`]; this module holds
//! the traits a source implements and the input rules every way into the
//! module goes through.

pub mod chains;
pub mod engine;
pub mod exchange_import;
#[cfg(test)]
mod exchange_import_tests;
pub mod exchange_sync;
#[cfg(test)]
mod exchange_sync_tests;
pub mod prices;
pub mod protocols;
pub mod rpc;
pub mod source;
pub mod sources;
pub mod tokens;
pub mod tracking;
#[cfg(test)]
mod tracking_tests;
pub mod validate;

pub use protocols::{ProtocolAdapter, ProtocolRequest, Registry as ProtocolRegistry};
pub use source::{FetchPage, FetchRequest, Source};
pub use validate::validate_section;
