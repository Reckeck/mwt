//! Workflow reactions to automation and goal events. Each calls the workflow
//! service the way the direct call does today; the event carries the id only.

use std::collections::HashSet;

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
    let zone = vault
        .doc
        .settings
        .schedule_timezone
        .clone()
        .unwrap_or_else(|| "UTC".into());
    vault
        .retry_workflow_run(&retried.id, now_secs(), &zone)
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

/// A goal was resumed: the stopped workflows that save into it start again.
pub fn on_goal_resumed(vault: &mut UnlockedVault, event: &DomainEvent) -> Result<()> {
    let DomainEvent::GoalResumed(resumed) = event else {
        return Ok(());
    };
    let goal = goal_of(vault, &resumed.id)?;
    for workflow in stopped_savers(vault, &goal) {
        vault.set_workflow_active(&workflow, true, now_secs())?;
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

/// Stopped workflows that pay into a wallet of the goal no other goal uses:
/// the mirror of `goal_savers`, which sees active workflows only.
fn stopped_savers(vault: &UnlockedVault, goal: &Goal) -> Vec<String> {
    let others: HashSet<&String> = vault
        .doc
        .goals
        .iter()
        .filter(|other| other.id != goal.id)
        .flat_map(|other| &other.scope.store_ids)
        .collect();
    vault
        .doc
        .workflows
        .iter()
        .filter(|workflow| !workflow.active && workflow.automation.is_none())
        .filter(|workflow| {
            crate::workflow::Graph::parse(&workflow.graph).is_ok_and(|graph| {
                graph.nodes.iter().any(|node| {
                    node.kind == "action.transaction"
                        && node
                            .params
                            .get("to")
                            .and_then(|to| to.as_str())
                            .is_some_and(|to| {
                                goal.scope.store_ids.iter().any(|store| store == to)
                                    && !others.contains(&to.to_string())
                            })
                })
            })
        })
        .map(|workflow| workflow.id.clone())
        .collect()
}
