//! Shim of the `goals` area: the rules moved to `crate::planning::goals`, the scenarios and the
//! progress queries to `crate::usecases::planning::goals` (spec core-strangler, D1).

pub use crate::planning::goals::{
    GoalCalendar, GoalCalendarMonth, GoalDeleted, GoalProgress, RealismIssue,
};
