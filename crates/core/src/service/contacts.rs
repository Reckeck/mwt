//! Shim of the `contacts` area: moved to `crate::household::contacts` (spec core-strangler, D1).

#[cfg(test)]
pub use crate::household::contacts::DebtTermsDraft;
pub use crate::household::contacts::{ContactDraft, TurnoverRow};
