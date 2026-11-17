//! The typed IPC layer: the error type commands return and, later, the
//! run-time helpers around it.

mod error;

// Nothing calls into this module until the command batches (T2-T6) convert.
#[expect(unused_imports, reason = "used from T2-T6")]
pub use error::IpcError;
