//! Workflow reactions to automation and goal events. Each calls the workflow
//! service the way the direct call does today; the event carries what the
//! direct call gets besides the vault (the device zone, the stopped ids).

use crate::error::Result;
use crate::kernel::events::DomainEvent;
use crate::service::Session;

/// An automation was deleted: its workflow goes with its runs.
pub fn on_automation_deleted(vault: &mut Session, event: &DomainEvent) -> Result<()> {
    match event {
        DomainEvent::AutomationDeleted(deleted) => {
            crate::usecases::workflow::records::delete_workflow(vault, &deleted.id)
        }
        _ => Ok(()),
    }
}

/// A refused run of an automation (the event names the run) is run again now.
pub fn on_automation_retried(vault: &mut Session, event: &DomainEvent) -> Result<()> {
    let DomainEvent::AutomationRetried(retried) = event else {
        return Ok(());
    };
    crate::usecases::workflow::runs::retry_workflow_run(
        vault,
        &retried.id,
        retried.at,
        &retried.zone,
    )
    .map(drop)
}

/// A goal was paused: the workflows that only save into it (the event names
/// them, like `GoalResumed`) stop.
pub fn on_goal_paused(vault: &mut Session, event: &DomainEvent) -> Result<()> {
    let DomainEvent::GoalPaused(paused) = event else {
        return Ok(());
    };
    for workflow in &paused.workflow_ids {
        crate::usecases::workflow::records::set_workflow_active(vault, workflow, false, paused.at)?;
    }
    Ok(())
}

/// A goal was resumed: the workflows its pause stopped (the event names them,
/// like `GoalDeleted.workflow_ids`) start again; one switched off by hand
/// before the pause stays off.
pub fn on_goal_resumed(vault: &mut Session, event: &DomainEvent) -> Result<()> {
    let DomainEvent::GoalResumed(resumed) = event else {
        return Ok(());
    };
    for workflow in &resumed.workflow_ids {
        // A workflow deleted since then has nothing to switch back on.
        if crate::workflow::service::records::workflow(&vault.doc, workflow).is_ok() {
            crate::usecases::workflow::records::set_workflow_active(
                vault, workflow, true, resumed.at,
            )?;
        }
    }
    Ok(())
}
