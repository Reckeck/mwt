//! Tests of `mwt-sync` items that need something only the facade has (`UnlockedVault`,
//! `service::test_support`). They moved here from the crate with their assertions unchanged
//! (crate-split D6).

mod apply_tests;
mod sync_pins_tests;
mod transfer;
