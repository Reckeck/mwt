//! Scenarios of the automations area: each decides in `planning::automations`
//! (and in `automation_rules` for the rules that read the workflow graph),
//! then ends in one `vault.transact`. Deleting and retrying announce their
//! event from inside that transact; the workflow reactions run after the
//! commit (`usecases::react`). The schedule marks and `skip_workflow_run`
//! stay direct calls of the workflow service.

use std::collections::HashSet;

use super::automation_rules::{
    check_draft_moment, detach, manual_run_record, new_automation_workflow, rewrite_automation,
    simulation_report,
};
use crate::Amount;
use crate::calendar;
use crate::error::{Error, Result};
use crate::kernel::events::{AutomationDeleted, AutomationRetried, DomainEvent};
use crate::model::planning::{Automation, AutomationRun, draft_to_transaction};
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::planning::automation_plan as plan;
use crate::planning::automations::{self as rules, AutomationDraft, AutomationSimulation};
use crate::pricing::history::secs_or_now;
use crate::service::UnlockedVault;
use crate::usecases::react::{ReactionFailure, react};
use crate::workflow::automation as graph_of;
use crate::workflow::model::Workflow;

pub fn automations(vault: &UnlockedVault) -> Vec<Automation> {
    plan::automations(&vault.books())
}

pub fn automation(vault: &UnlockedVault, id: &str) -> Result<Automation> {
    vault
        .doc
        .workflows
        .iter()
        .find(|workflow| workflow.id == id)
        .and_then(|workflow| plan::automation_in(&vault.books(), workflow))
        .ok_or_else(|| Error::Missing(format!("automation {id}")))
}

/// Reject a draft that could never run: a bad schedule, or a template
/// that fails the transaction rules on its own.
fn check_automation(vault: &UnlockedVault, draft: &AutomationDraft) -> Result<()> {
    rules::check_draft_form(draft)?;
    vault.check_property_link(draft.property_id.as_deref())?;
    let starts = check_draft_moment(draft)?;
    // The probe is dated at the start, which may be months ahead: it is
    // checked against the rules, not against today.
    let mut probe = draft_to_transaction(&draft.template, "", "", &draft.starts_at);
    let reference = starts.max(secs_or_now(&now_iso()));
    vault.prepare_transaction_at(&mut probe, reference * 1000)
}

pub fn create_automation(vault: &mut UnlockedVault, draft: AutomationDraft) -> Result<Automation> {
    check_automation(vault, &draft)?;
    let id = uuid::Uuid::new_v4().to_string();
    let workflow = new_automation_workflow(&draft, &id, &now_iso())?;
    vault.wf_set_mark(
        &id,
        &graph_of::schedule_key(),
        &graph_of::start_mark(&draft.spec()),
    );
    vault.transact(|tx| {
        tx.doc_mut().workflows.push(workflow);
        Ok(())
    })?;
    automation(vault, &id)
}

/// Change what an automation does: its graph is built again, the old one
/// kept as a version. Dates already processed stay processed.
pub fn update_automation(
    vault: &mut UnlockedVault,
    id: &str,
    draft: AutomationDraft,
) -> Result<Automation> {
    update_automation_at(vault, id, draft, calendar::now_secs())
}

/// [`update_automation`] with the edit moment given by the caller.
pub fn update_automation_at(
    vault: &mut UnlockedVault,
    id: &str,
    draft: AutomationDraft,
    edited_at: i64,
) -> Result<Automation> {
    check_automation(vault, &draft)?;
    let schedule_changed = rules::schedule_changed(&vault.doc, id, &draft)?;
    // A start moved later holds the schedule back until then.
    let key = graph_of::schedule_key();
    let start = graph_of::start_mark(&draft.spec());
    if vault
        .wf_mark(id, &key)
        .is_none_or(|mark| secs_or_now(&mark) < secs_or_now(&start))
    {
        vault.wf_set_mark(id, &key, &start);
    }
    // A changed schedule plans from the edit on: nothing is back-filled.
    if schedule_changed
        && vault
            .wf_mark(id, &key)
            .is_none_or(|mark| secs_or_now(&mark) < edited_at)
    {
        vault.wf_set_mark(id, &key, &calendar::iso(edited_at));
    }
    let now = now_iso();
    vault.transact(|tx| {
        let workflow = tx
            .doc_mut()
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == id && workflow.automation.is_some())
            .ok_or_else(|| Error::Missing(format!("automation {id}")))?;
        rewrite_automation(workflow, &draft, &now)
    })?;
    automation(vault, id)
}

/// Remove an automation and its run history. Transactions it created stay.
pub fn delete_automation(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    automation(vault, id)?;
    announce(vault, AutomationDeleted { id: id.to_string() }.into())
}

/// Commit `event` (it touches the document, so it is delivered), then run its
/// reactions; the first one that failed is the error of the scenario.
pub(super) fn announce(vault: &mut UnlockedVault, event: DomainEvent) -> Result<()> {
    vault.transact(|tx| {
        tx.emit(event.clone());
        tx.touch();
        Ok(())
    })?;
    first_failure(react(vault, vec![event]))
}

fn first_failure(failures: Vec<ReactionFailure>) -> Result<()> {
    failures
        .into_iter()
        .next()
        .map_or(Ok(()), |failure| Err(failure.error))
}

/// Make an automation an ordinary workflow: the graph stays and is edited
/// in the editor from now on; the form no longer shows it.
pub fn detach_automation(vault: &mut UnlockedVault, id: &str) -> Result<Workflow> {
    let now = now_iso();
    vault.transact(|tx| {
        let workflow = tx
            .doc_mut()
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == id && workflow.automation.is_some())
            .ok_or_else(|| Error::Missing(format!("automation {id}")))?;
        detach(workflow, &now);
        Ok(workflow.clone())
    })
}

/// Emulate the automations over the next `days` (one of them, or all):
/// day by day on a sandbox copy, so each run sees the balance the earlier
/// ones left, exactly as the scheduler would. The vault is not touched.
pub fn simulate_automations(
    vault: &UnlockedVault,
    automation_id: Option<&str>,
    days: u32,
    now: i64,
    zone: &str,
) -> Result<AutomationSimulation> {
    if let Some(id) = automation_id {
        automation(vault, id)?;
    }
    let days = days.clamp(1, rules::MAX_SIMULATION_DAYS);
    let mut sandbox = vault.sandbox();
    let known: HashSet<String> = sandbox
        .doc
        .workflow_runs
        .iter()
        .map(|run| run.id.clone())
        .collect();
    let before = sandbox.doc.holdings.clone();
    for day in 0..=i64::from(days) {
        sandbox.run_due_workflows(now + day * 86_400, zone)?;
    }
    Ok(simulation_report(
        &sandbox.doc,
        &known,
        &before,
        automation_id,
        now,
        days,
    ))
}

/// Funds check, then the transaction. A shortfall is an `INSUFFICIENT_FUNDS`
/// error. Returns the prepared transaction; the caller writes it.
fn checked_transaction(
    vault: &UnlockedVault,
    mut tx: Transaction,
    now: i64,
) -> Result<Transaction> {
    if let Some(reason) = rules::funds_shortfall(&vault.doc, &tx, now) {
        return Err(rules::funds_error(&vault.doc, reason));
    }
    vault.prepare_transaction_at(&mut tx, now * 1000)?;
    Ok(tx)
}

/// Create the transaction of an automation now ("Create now"), with
/// the same funds check as the schedule.
pub fn run_automation(vault: &mut UnlockedVault, id: &str, now: i64) -> Result<Transaction> {
    let automation = automation(vault, id)?;
    let planned_at = calendar::iso(now);
    let run_id = uuid::Uuid::new_v4().to_string();
    let made = rules::manual_run_transaction(&automation, &planned_at, &run_id);
    let saved = checked_transaction(vault, made, now)?;
    let record = manual_run_record(&run_id, id, &planned_at, &saved.meta().id);
    // The holdings follow the booked transaction and rebasing rewrites the
    // stored `before` of corrections, so the booking is applied and the
    // holdings recomputed before the one commit. The booking emits no events,
    // so none is dropped by `run`.
    vault.run(|tx| {
        let doc = tx.doc_mut();
        doc.transactions.push(saved.clone());
        doc.workflow_runs.push(record);
        Ok(())
    })?;
    vault.recompute_holdings();
    vault.transact(|tx| {
        tx.touch();
        Ok(())
    })?;
    Ok(saved)
}

/// Retry a refused run: funds are checked at the current moment and the
/// transaction is dated now.
pub fn retry_automation_run(
    vault: &mut UnlockedVault,
    run_id: &str,
    now: i64,
    zone: &str,
) -> Result<Transaction> {
    if !vault.doc.workflow_runs.iter().any(|run| run.id == run_id) {
        return Err(Error::Missing(format!("workflow run {run_id}")));
    }
    let known: HashSet<String> = vault
        .doc
        .transactions
        .iter()
        .map(|tx| tx.meta().id.clone())
        .collect();
    let retried = AutomationRetried {
        id: run_id.to_string(),
        zone: zone.to_string(),
        at: now,
    };
    announce(vault, retried.into())?;
    vault
        .doc
        .transactions
        .iter()
        .find(|tx| !known.contains(&tx.meta().id))
        .cloned()
        .ok_or_else(|| Error::InvalidInput("the run made no operation".into()))
}

/// Dismiss a refused run.
pub fn skip_automation_run(vault: &mut UnlockedVault, run_id: &str) -> Result<AutomationRun> {
    let run = vault.skip_workflow_run(run_id)?;
    plan::automation_run_of(&vault.doc, &run)
        .ok_or_else(|| Error::Missing(format!("automation run {run_id}")))
}

/// Put in the real amount of an operation booked approximately: it is
/// corrected with the usual checks and counts toward the next estimate.
pub fn confirm_estimated_amount(
    vault: &mut UnlockedVault,
    tx_id: &str,
    amount: Amount,
) -> Result<Transaction> {
    let corrected = rules::confirmed_transaction(&vault.doc, tx_id, amount)?;
    vault.update_transaction(tx_id, corrected)?;
    clear_estimated(vault, tx_id)
}

/// Drop the «approximate» mark of an operation; saved when it changed.
fn clear_estimated(vault: &mut UnlockedVault, tx_id: &str) -> Result<Transaction> {
    vault.transact(|tx| {
        let index = tx
            .doc()
            .transactions
            .iter()
            .position(|known| known.meta().id == tx_id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        let mut made = tx.doc().transactions[index].clone();
        if rules::clear_estimated(&mut made) {
            tx.doc_mut().transactions[index] = made.clone();
        }
        Ok(made)
    })
}
