//! Shim of the `automation_card` area: the facts built from the document moved to
//! `crate::planning::automation_card`, the scenarios that read workflow graphs to
//! `crate::usecases::planning::automation_card` (spec core-strangler, D1).

pub use crate::planning::automation_card::{CardAction, CardMoney, CardTrigger, JournalStatus};
