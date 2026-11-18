//! Workflow reactions to automation and goal events. Each calls the workflow
//! service the way the direct call does today; the event carries what the
//! direct call gets besides the vault (the device zone, the stopped ids).

use crate::calendar::now_secs;
use crate::error::Result;
use crate::kernel::events::DomainEvent;
use crate::model::planning::Goal;
use crate::service::UnlockedVault;

/// An automation was deleted: its workflow goes with its runs.
pub fn on_automation_deleted(vault: &mut UnlockedVault, event: &DomainEvent) -> Result<()> {
    match event {
        DomainEvent::AutomationDeleted(deleted) => vault.delete_workflow(&deleted.id),
        _ => Ok(()),
    }
}

/// A refused run of an automation (the event names the run) is run again now.
pub fn on_automation_retried(vault: &mut UnlockedVault, event: &DomainEvent) -> Result<()> {
    let DomainEvent::AutomationRetried(retried) = event else {
        return Ok(());
    };
    vault
        .retry_workflow_run(&retried.id, now_secs(), &retried.zone)
        .map(drop)
}

/// A goal was paused: the workflows that only save into it stop.
pub fn on_goal_paused(vault: &mut UnlockedVault, event: &DomainEvent) -> Result<()> {
    let DomainEvent::GoalPaused(paused) = event else {
        return Ok(());
    };
    let goal = goal_of(vault, &paused.id)?;
    for workflow in vault.goal_savers(&goal) {
        vault.set_workflow_active(&workflow, false, now_secs())?;
    }
    Ok(())
}

/// A goal was resumed: the workflows its pause stopped (the event names them,
/// like `GoalDeleted.workflow_ids`) start again; one switched off by hand
/// before the pause stays off.
pub fn on_goal_resumed(vault: &mut UnlockedVault, event: &DomainEvent) -> Result<()> {
    let DomainEvent::GoalResumed(resumed) = event else {
        return Ok(());
    };
    for workflow in &resumed.workflow_ids {
        // A workflow deleted since then has nothing to switch back on.
        if vault.workflow(workflow).is_ok() {
            vault.set_workflow_active(workflow, true, now_secs())?;
        }
    }
    Ok(())
}

fn goal_of(vault: &UnlockedVault, id: &str) -> Result<Goal> {
    vault
        .doc
        .goals
        .iter()
        .find(|goal| goal.id == id)
        .cloned()
        .ok_or_else(|| crate::error::Error::Missing(format!("goal {id}")))
}
