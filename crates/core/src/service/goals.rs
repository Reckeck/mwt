//! Shim of the `goals` area: the rules moved to `crate::planning::goals`, the scenarios and the
//! progress queries to `crate::usecases::planning::goals` (spec core-strangler, D1).

pub use crate::planning::goals::{
    GoalCalendar, GoalCalendarMonth, GoalDeleted, GoalDraft, GoalProgress, RealismIssue,
};

use crate::Amount;
use crate::error::Result;
use crate::model::planning::Goal;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Goals by importance (more stars first), then by deadline.
        pub fn goals(&self) -> Vec<Goal>
            => crate::planning::goals::goals @doc;
        pub fn create_goal(&mut self, draft: GoalDraft) -> Result<Goal>
            => crate::usecases::planning::goals::create_goal;
        pub fn update_goal(&mut self, id: &str, draft: GoalDraft) -> Result<Goal>
            => crate::usecases::planning::goals::update_goal;
        /// Delete a goal (New-Goal 5): the money stays in its wallets, its
        /// envelopes go back to "safe to spend", the workflows that only put
        /// money aside into it stop. Its files go with the next open of the vault
        /// unless [`restore_goal`] brings it back first.
        pub fn delete_goal(&mut self, id: &str, now: i64) -> Result<GoalDeleted>
            => crate::usecases::planning::goals::delete_goal;
        /// Undo [`delete_goal`]: the goal, its envelopes and its workflows.
        pub fn restore_goal(&mut self, deleted: GoalDeleted, now: i64) -> Result<Goal>
            => crate::usecases::planning::goals::restore_goal;
        /// Average of `income − fixed − daily` over the last non-empty months
        /// before the current one, in the budget currency; `None` without any.
        pub fn free_surplus_avg(&self, now: i64, zone: &str) -> Option<Amount>
            => crate::usecases::planning::goals::free_surplus_avg @books;
        pub fn goal_progress(&self, now: i64, zone: &str) -> Vec<GoalProgress>
            => crate::usecases::planning::goals::goal_progress @books;
        pub fn goal_calendar(&self, year: i32, now: i64, zone: &str) -> Result<GoalCalendar>
            => crate::usecases::planning::goals::goal_calendar @books;
    }
    // forward:end

    // Read by the emergency and debt areas until they move.
    forward! {}
}
