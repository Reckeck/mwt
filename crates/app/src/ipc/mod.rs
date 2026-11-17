//! The typed IPC layer: the error type commands return and, later, the
//! run-time helpers around it.

// Nothing calls into this module until the command batches (T2-T6) convert.
#![allow(dead_code, unused_imports)]

mod error;

pub use error::IpcError;
