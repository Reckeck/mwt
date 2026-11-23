//! Tests of `mwt-vault` items that need something only the facade has (`Session`, the
//! full fixture document, the workflow graph version). They moved here from the crate with their
//! assertions unchanged (crate-split D6).

mod adopt;
mod kernel_collection;
mod kernel_golden;
mod storage;
mod v4_fixtures_gen;
mod vault_io;
