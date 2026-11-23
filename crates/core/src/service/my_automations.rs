//! Shim of the `my_automations` area: the rows built from the document moved to
//! `crate::planning::my_automations`, the scenarios to `crate::usecases::planning::my_automations`
//! (spec core-strangler, D1).

pub use crate::planning::my_automations::{MineItem, MineKind};
