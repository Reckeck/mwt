//! Shim of the `scenarios` area: the rules moved to `crate::planning::scenarios`, the
//! scenarios to `crate::usecases::planning::scenarios` (spec core-strangler, D1).

pub use crate::planning::scenarios::{
    CalcScenarioDeleted, CalcScenarioDraft, INCOME_MONTHS, ScenarioPrefill, planned_balance,
};

use crate::error::Result;
use crate::ipc_contract::GoalInput;
use crate::model::planning::{CalcScenario, ScenarioSource};

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Saved scenarios, the last changed first.
        pub fn calc_scenarios(&self) -> Vec<CalcScenario>
            => crate::planning::scenarios::calc_scenarios @doc;
        /// Start amount, rate and contribution from the user's data (plan A,
        /// decisions 7 and 9). Nothing is saved.
        pub fn scenario_prefill(&self, source: &ScenarioSource, currency: &str, now: i64, zone: &str) -> Result<ScenarioPrefill>
            => crate::planning::scenarios::scenario_prefill @books;
        /// Save a scenario: a new one when `draft.id` is `None`, else the edited one.
        pub fn save_calc_scenario(&mut self, draft: CalcScenarioDraft) -> Result<CalcScenario>
            => crate::usecases::planning::scenarios::save_calc_scenario;
        /// Delete a scenario; goals made from it stay and stop following it.
        pub fn delete_calc_scenario(&mut self, id: &str) -> Result<CalcScenarioDeleted>
            => crate::usecases::planning::scenarios::delete_calc_scenario;
        /// Undo [`delete_calc_scenario`]: the scenario and its goal links.
        pub fn restore_calc_scenario(&mut self, deleted: CalcScenarioDeleted) -> Result<CalcScenario>
            => crate::usecases::planning::scenarios::restore_calc_scenario;
        /// A goal to reach what the scenario ends with, for the goal form to
        /// fill in (plan A, decision 12). Nothing is saved.
        pub fn goal_from_scenario(&self, id: &str, now: i64, zone: &str) -> Result<GoalInput>
            => crate::usecases::planning::scenarios::goal_from_scenario @doc;
    }
    // forward:end

    // Read by the dashboard, the relations and the goal checks until they move.
    forward! {
        pub(crate) fn find_calc_scenario(&self, id: &str) -> Option<&CalcScenario>
            => crate::planning::scenarios::find_calc_scenario @doc;
    }
}
