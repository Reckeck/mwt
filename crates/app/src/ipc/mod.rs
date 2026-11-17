//! The typed IPC layer: the error type commands return and, later, the
//! run-time helpers around it.

pub(crate) mod commands;
mod error;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "first callers are the emit sites (T21)")
)]
pub(crate) mod events;
#[expect(dead_code, reason = "trial: no command takes the extractor yet")]
mod extract;
pub mod opener;
#[expect(dead_code, reason = "first caller is the #[ipc] expansion (T9, T10)")]
pub mod run;

pub use error::IpcError;
#[expect(unused_imports, reason = "first user is the pilot module (T10)")]
pub use mwt_app_macros::ipc;
