//! Shim of the `notifications` area: the rules moved to `crate::planning::notifications`, the
//! scenarios to `crate::usecases::planning::notifications` (spec core-strangler, D1).

pub use crate::model::notify::{notice, notice_title};
