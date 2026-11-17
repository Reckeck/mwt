//! The typed IPC layer: the error type commands return and, later, the
//! run-time helpers around it.

mod error;
pub mod opener;

pub use error::IpcError;
