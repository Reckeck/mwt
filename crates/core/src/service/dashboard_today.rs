//! Shim of the `dashboard_today` area: moved to `crate::reports::dashboard_today` (spec core-strangler, D1).

pub use crate::reports::dashboard_today::{
    DashboardToday, ScenarioStatus, TODAY_TODOS, TodayTodo, TodayTodoKind,
};

use super::UnlockedVault;
use super::forward::forward;
use crate::error::Result;
use crate::model::vault::DashPeriod;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Plan against fact for one saved scenario: the fact is its "From your data"
        /// wallets, or else the wallets of the goal that follows it.
        pub fn scenario_status(&self, id: &str, now: i64, zone: &str) -> Result<ScenarioStatus>
            => crate::reports::dashboard_today::scenario_status @books;
        /// Everything "Today" shows, in one request (plan H5).
        pub fn dashboard_today(&self, period: DashPeriod, now: i64, zone: &str) -> DashboardToday
            => crate::reports::dashboard_today::dashboard_today @books;
    }
    // forward:end

    // The tool forwards public methods only; the crate-private delegate is kept by hand, for the tests.
    #[cfg(test)]
    forward! {
        pub(crate) fn all_today_todos(&self, now: i64, zone: &str, currency: Option<&str>) -> Vec<TodayTodo>
            => crate::reports::dashboard_today::all_today_todos @books;
    }
}
