//! Scenarios of the saved-calculations area: each decides in
//! `planning::scenarios`, then ends in one `vault.transact`. The goal a
//! scenario makes is built here because its form (`GoalInput`) belongs to the
//! use-case layer.

use chrono::{Months, NaiveDate};

use crate::calc::invest;
use crate::calendar::{self, Calendar};
use crate::error::{Error, Result};
use crate::ipc_contract::GoalInput;
use crate::model::planning::{CalcScenario, Goal};
use crate::model::store::StoreScope;
use crate::model::vault::{VaultDoc, now_iso};
use crate::planning::scenarios::{
    CalcScenarioDeleted, CalcScenarioDraft, SCENARIO_GOAL_PRIORITY, find_calc_scenario, per_month,
    plan_delete_calc_scenario, plan_restore_calc_scenario, plan_save_calc_scenario,
};
use crate::service::Session;

/// Save a scenario: a new one when `draft.id` is `None`, else the edited one.
pub fn save_calc_scenario(vault: &mut Session, draft: CalcScenarioDraft) -> Result<CalcScenario> {
    let is_new = draft.id.is_none();
    let saved = plan_save_calc_scenario(
        &vault.doc,
        draft,
        uuid::Uuid::new_v4().to_string(),
        &now_iso(),
    )?;
    vault.transact(|tx| {
        let mut scenarios = tx.collection::<CalcScenario>();
        if is_new {
            scenarios.insert(saved.clone())?;
        } else {
            scenarios.update(&saved.id, |slot| *slot = saved.clone())?;
        }
        Ok(saved)
    })
}

/// Delete a scenario; goals made from it stay and stop following it.
pub fn delete_calc_scenario(vault: &mut Session, id: &str) -> Result<CalcScenarioDeleted> {
    let deleted = plan_delete_calc_scenario(&vault.doc, id)?;
    vault.transact(|tx| {
        tx.collection::<CalcScenario>().remove(id)?;
        for goal_id in &deleted.goal_ids {
            tx.collection::<Goal>()
                .update(goal_id, |goal| goal.scenario_id = None)?;
        }
        Ok(())
    })?;
    Ok(deleted)
}

/// Undo [`delete_calc_scenario`]: the scenario and its goal links.
pub fn restore_calc_scenario(
    vault: &mut Session,
    deleted: CalcScenarioDeleted,
) -> Result<CalcScenario> {
    let relink = plan_restore_calc_scenario(&vault.doc, &deleted)?;
    let CalcScenarioDeleted { scenario, .. } = deleted;
    vault.transact(|tx| {
        tx.collection::<CalcScenario>().insert(scenario.clone())?;
        for goal_id in &relink {
            tx.collection::<Goal>()
                .update(goal_id, |goal| goal.scenario_id = Some(scenario.id.clone()))?;
        }
        Ok(scenario)
    })
}

/// A goal to reach what the scenario ends with, for the goal form to
/// fill in (plan A, decision 12). Nothing is saved.
pub fn goal_from_scenario(doc: &VaultDoc, id: &str, now: i64, zone: &str) -> Result<GoalInput> {
    let scenario =
        find_calc_scenario(doc, id).ok_or_else(|| Error::Missing(format!("scenario {id}")))?;
    let result = invest::simulate(&scenario.input)?;
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let deadline = today
        .checked_add_months(Months::new(scenario.input.term_months))
        .unwrap_or(today);
    let at_midnight = |date: NaiveDate| calendar::iso(cal.at_local(date, 0));
    let first_stage = scenario
        .input
        .stages
        .iter()
        .min_by_key(|stage| stage.from_month);
    Ok(GoalInput {
        name: scenario.name.clone(),
        active: true,
        target_amount: result.final_nominal,
        currency: scenario.input.currency.trim().to_lowercase(),
        starts_at: at_midnight(today),
        deadline: Some(at_midnight(deadline)),
        priority: SCENARIO_GOAL_PRIORITY,
        monthly_commitment: first_stage.map(|stage| per_month(stage).round(2)),
        scope: scenario
            .source
            .as_ref()
            .map_or_else(StoreScope::default, |source| source.scope.clone()),
        scenario_id: Some(scenario.id.clone()),
        expected_return: Some(scenario.input.rate_pct).filter(|rate| rate.is_positive()),
    })
}
