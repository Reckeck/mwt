//! Shim of the `debts` area: moved to `crate::household::debts` (spec core-strangler, D1).

pub use crate::household::debts::{DebtLoad, LoanClass, LoanStatus, classify_loan};
