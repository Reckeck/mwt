//! The invariant pass of sync (stage M) and the small repairs around a merge: pure rules over a
//! `VaultDoc`. Nothing here reads the clock, the tracker or the vault; the scenarios that run the
//! rules, stamp the result and record it in the batch are in `usecases::sync::repair`.

pub mod bad_record;
pub mod keep;
#[allow(clippy::module_inception)]
pub mod repair;
pub mod repair_cash;
pub mod repair_classes;
pub mod repair_events;
pub mod repair_imports;
pub mod repair_natural;
pub mod repair_report;
pub mod repair_restore;
pub mod repoint;
