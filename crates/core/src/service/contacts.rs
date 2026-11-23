//! Shim of the `contacts` area: moved to `crate::household::contacts` (spec core-strangler, D1).

pub use crate::household::contacts::{
    ContactBalance, ContactDraft, ContactSummary, ContactTurnover, DebtTermsDraft, TurnoverRow,
};
