//! IPC types of the crypto module, one file per domain (stage 2, spec E0).
//!
//! `ipc_contract.rs` is a list the whole stage shares, so E0 declares every
//! domain of the module here once and no later spec has to touch a shared
//! file. Each file below names the spec that owns it: that spec writes its
//! DTOs, its commands in `crates/app/src/commands/cryptotrack/<domain>.rs`
//! and its own `export!` block in `crates/core/src/bin/gen_types.rs`.
//!
//! E0 names no DTO of its own: a shape invented before the screen that uses
//! it would only be rewritten. What E0 fixes is the command names (they are
//! registered in `generate_handler!`), the files and who owns them.

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
