//! Shim of the `agenda` area: the rules moved to `crate::planning::agenda`, the scenarios to
//! `crate::usecases::planning::agenda` (spec core-strangler, D1).

pub use crate::planning::agenda::{PriceAlertDraft, UpcomingItem, UpcomingKind};
