//! Commands of the crypto module, one file per domain (stage 2, spec E0).
//!
//! `generate_handler!` in `crates/app/src/lib.rs` is a list the whole stage
//! shares, so E0 registers every command of the module there once: a later
//! spec writes only the body of its own file and never edits a shared list.
//!
//! Until its owner spec arrived a command answered `UNSUPPORTED` and named
//! the spec that fills it; spec D0 wrote the last of those bodies, so there
//! is no stub left here and no helper for one.

pub mod accounts;
pub mod links;
pub mod network;
pub mod operations;
pub mod positions;
pub mod relations;
pub mod sync;
pub mod tokens;
pub mod transition;
pub mod wipe;
