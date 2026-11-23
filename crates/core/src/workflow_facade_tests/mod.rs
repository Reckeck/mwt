//! Tests of `workflow/` items that need the vault (`Session` and its fixtures). They moved here
//! from the `mwt-workflow` crate with their assertions unchanged so that crate holds pure tests only
//! (crate-split T16). The files are named `<stem>_tests.rs`.

mod actions_tests;
mod dates_tests;
mod effect_tests;
