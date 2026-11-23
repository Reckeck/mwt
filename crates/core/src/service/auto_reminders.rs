//! Shim of the `auto_reminders` area: the rules moved to `crate::planning::auto_reminders`, the
//! scenarios to `crate::usecases::planning::auto_reminders` (spec core-strangler, D1).

pub use crate::planning::auto_reminders::category_of;
pub(crate) use crate::planning::auto_reminders::source_identity;
