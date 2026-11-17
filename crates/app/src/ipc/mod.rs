//! The typed IPC layer: the error type commands return and, later, the
//! run-time helpers around it.

mod error;
mod extract;
pub mod opener;
#[expect(dead_code, reason = "first caller is the #[ipc] expansion (T9, T10)")]
pub mod run;

pub use error::IpcError;
