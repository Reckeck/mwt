//! Platform-neutral half of vault file handling (the Tauri half is
//! `crates/app/src/platform_fs`).
//!
//! The working vault is always a local file ([`local`]): atomic replace,
//! the instance lock and `.bak` slots need a directory next to the file, which
//! a document picked through Android's Storage Access Framework does not have.
//! A picked document is therefore either copied in ([`import`]) or kept as an
//! external mirror the vault is copied to after every save ([`mirror`]).
//! [`location`] tells a path from such a document. See `docs/storage-platforms.md`.

pub mod backups;
pub mod delete;
pub mod import;
pub mod local;
pub mod location;
pub mod mirror;

pub use location::{VaultLocation, display_name_of};
pub use mirror::{SyncAction, VaultMirror};
