#![warn(unreachable_pub)]
//! `mwt-vault`: the encrypted vault file and everything about keeping it: the file codec
//! and the frozen v1-v4 schemas (`storage`), vault files, copies, backups and mirrors
//! (`vault_io`), the JSON stores outside the vault (`jsonstore`, `app_dirs`) and the kernel
//! of the open vault (`kernel`: `Write`, `Tx`, `Collection<T>`, `Persistence`). It depends
//! on `mwt-base`, `mwt-crypto` and `mwt-model` only. The modules are public.

pub mod app_dirs;
pub mod jsonstore;
pub mod kernel;
pub mod storage;
pub mod vault_io;

pub use storage::VaultFile;
