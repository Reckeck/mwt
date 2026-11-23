//! Shim of the `interest` area: moved to `crate::money::interest` (spec core-strangler, D1).
//! The scenarios that write terms, key rates and interest live in
//! `usecases::money::interest`.

pub use crate::money::interest::PdsReport;
