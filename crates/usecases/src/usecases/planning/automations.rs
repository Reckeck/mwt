//! Scenarios of the automations area: each decides in `planning::automations`
//! (and in `automation_rules` for the rules that read the workflow graph),
//! then ends in one `vault.transact`. Deleting and retrying announce their
//! event from inside that transact; the workflow reactions run after the
//! commit (`usecases::react`). The schedule marks and `skip_workflow_run`
//! stay direct calls of the workflow service.

use std::collections::HashSet;

use super::automation_rules::{
    Confirmation, check_draft_moment, detach, manual_run_record, new_automation_workflow,
    rewrite_automation, simulation_report,
};
use crate::error::{Error, Result};
use crate::kernel::events::{AutomationDeleted, AutomationRetried, DomainEvent};
use crate::model::planning::{Automation, AutomationRun, draft_to_transaction};
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::planning::automation_plan as plan;
use crate::planning::automations::{self as rules, AutomationDraft, AutomationSimulation};
use crate::service::Session;
use crate::time::secs_or_now;
use crate::usecases::planning::distribution::{DistributionPreview, preview_rule};
use crate::usecases::react::{ReactionFailure, react};
use crate::usecases::workflow::runs::book_manual_run;
use crate::workflow::automation as graph_of;
use crate::workflow::model::{RunOutcome, Workflow};
use crate::{Amount, calendar};
use mwt_model::{OnOtherAmount, WaitKind};

pub fn automations(vault: &Session) -> Vec<Automation> {
    plan::automations(&vault.books())
}

pub fn automation(vault: &Session, id: &str) -> Result<Automation> {
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
fn check_automation(vault: &Session, draft: &AutomationDraft) -> Result<()> {
    rules::check_draft_form(draft)?;
    crate::usecases::household::property_links::check_property_link(
        vault,
        draft.property_id.as_deref(),
    )?;
    let starts = check_draft_moment(draft)?;
    // The probe is dated at the start, which may be months ahead: it is
    // checked against the rules, not against today.
    let mut probe = draft_to_transaction(&draft.template, "", "", &draft.starts_at);
    let reference = starts.max(secs_or_now(&now_iso()));
    crate::usecases::money::transactions::prepare_transaction_at(
        vault,
        &mut probe,
        reference * 1000,
    )
}

pub fn create_automation(vault: &mut Session, draft: AutomationDraft) -> Result<Automation> {
    check_automation(vault, &draft)?;
    let id = uuid::Uuid::new_v4().to_string();
    let workflow = new_automation_workflow(&draft, &id, &now_iso())?;
    crate::usecases::workflow::runs::wf_set_mark(
        vault,
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
    vault: &mut Session,
    id: &str,
    draft: AutomationDraft,
) -> Result<Automation> {
    update_automation_at(vault, id, draft, calendar::now_secs())
}

/// [`update_automation`] with the edit moment given by the caller.
pub fn update_automation_at(
    vault: &mut Session,
    id: &str,
    draft: AutomationDraft,
    edited_at: i64,
) -> Result<Automation> {
    check_automation(vault, &draft)?;
    let schedule_changed = rules::schedule_changed(&vault.doc, id, &draft)?;
    // A start moved later holds the schedule back until then.
    let key = graph_of::schedule_key();
    let start = graph_of::start_mark(&draft.spec());
    if crate::workflow::service::marks::mark(&vault.doc, id, &key)
        .is_none_or(|mark| secs_or_now(&mark) < secs_or_now(&start))
    {
        crate::usecases::workflow::runs::wf_set_mark(vault, id, &key, &start);
    }
    // A changed schedule plans from the edit on: nothing is back-filled.
    if schedule_changed
        && crate::workflow::service::marks::mark(&vault.doc, id, &key)
            .is_none_or(|mark| secs_or_now(&mark) < edited_at)
    {
        crate::usecases::workflow::runs::wf_set_mark(vault, id, &key, &calendar::iso(edited_at));
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
pub fn delete_automation(vault: &mut Session, id: &str) -> Result<()> {
    automation(vault, id)?;
    announce(vault, AutomationDeleted { id: id.to_string() }.into())
}

/// Commit `event` (it touches the document, so it is delivered), then run its
/// reactions; the first one that failed is the error of the scenario.
pub(super) fn announce(vault: &mut Session, event: DomainEvent) -> Result<()> {
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
pub fn detach_automation(vault: &mut Session, id: &str) -> Result<Workflow> {
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
    vault: &Session,
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
        crate::usecases::workflow::runs::run_due_workflows(&mut sandbox, now + day * 86_400, zone)?;
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

/// Create the transaction of an automation now ("Create now"): booked as the schedule books it,
/// with the same funds check and the template's period rule and distribution.
pub fn run_automation(vault: &mut Session, id: &str, now: i64) -> Result<Transaction> {
    let automation = automation(vault, id)?;
    let planned_at = calendar::iso(now);
    let run_id = uuid::Uuid::new_v4().to_string();
    let made = rules::manual_run_transaction(&automation, &planned_at, &run_id);
    let record = manual_run_record(&run_id, id, &planned_at, &made.meta().id);
    book_manual_run(vault, made, record, now)
}

/// Retry a refused run: funds are checked at the current moment and the
/// transaction is dated now.
pub fn retry_automation_run(
    vault: &mut Session,
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
pub fn skip_automation_run(vault: &mut Session, run_id: &str) -> Result<AutomationRun> {
    let run = crate::usecases::workflow::runs::skip_workflow_run(vault, run_id)?;
    plan::automation_run_of(&vault.doc, &run)
        .ok_or_else(|| Error::Missing(format!("automation run {run_id}")))
}

/// Put in the real amount of an operation booked approximately: it is
/// corrected with the usual checks and counts toward the next estimate.
pub fn confirm_estimated_amount(
    vault: &mut Session,
    tx_id: &str,
    amount: Amount,
) -> Result<Transaction> {
    let corrected = rules::confirmed_transaction(&vault.doc, tx_id, amount)?;
    crate::usecases::money::transactions::update_transaction(vault, tx_id, corrected)?;
    clear_estimated(vault, tx_id)
}

/// Drop the «approximate» mark of an operation; saved when it changed.
fn clear_estimated(vault: &mut Session, tx_id: &str) -> Result<Transaction> {
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

/// D11, I11: the rows of the automation's next payout under its template's rules, for the given or
/// the expected amount. Writes nothing.
pub fn preview_distribution_rule(
    vault: &Session,
    automation_id: &str,
    amount: Option<Amount>,
    now: i64,
) -> Result<DistributionPreview> {
    preview_rule(vault, &automation(vault, automation_id)?, amount, now)
}

/// D9, Q12, I12: answer "yes" to the confirmation of an automation run that asks, and book the
/// income at the confirmed `amount`, distributed by the template's rule as `mode` when the user
/// chose one. Until then nothing is booked. The answer is tried on a copy of the vault first:
/// a refusal (the rule still asks or asks too much, the money is short) leaves the question and
/// the document as they were. Returns the income as stored.
pub fn confirm_distribution(
    vault: &mut Session,
    run_id: &str,
    amount: Amount,
    mode: Option<OnOtherAmount>,
    now: i64,
    zone: &str,
) -> Result<Transaction> {
    if mode == Some(OnOtherAmount::Ask) {
        return Err(Error::InvalidInput(
            "a confirmation chooses how to distribute, it cannot ask again".into(),
        ));
    }
    if !amount.is_positive() {
        return Err(Error::InvalidInput("the amount must be above zero".into()));
    }
    let wait = vault
        .doc
        .workflow_waits
        .iter()
        .find(|wait| wait.run_id == run_id && wait.kind == WaitKind::Confirm)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("question of run {run_id}")))?;
    let automation = automation(vault, &wait.workflow_id)?;
    let has_rule = automation
        .operation_template_id
        .as_deref()
        .and_then(|id| {
            vault
                .doc
                .operation_templates
                .iter()
                .find(|known| known.id == id)
        })
        .is_some_and(|template| template.distribution.is_some());
    if !has_rule {
        return Err(Error::Missing("the template's distribution rule".into()));
    }
    let confirmation = Confirmation {
        workflow_id: wait.workflow_id.clone(),
        amount,
        mode,
    };
    answer_confirmed(&mut vault.sandbox(), &wait.id, &confirmation, now, zone)?;
    let income_id = answer_confirmed(vault, &wait.id, &confirmation, now, zone)?;
    vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == income_id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("income {income_id}")))
}

/// Answer "yes" with the confirmation riding the run; the id of the income the run booked.
fn answer_confirmed(
    vault: &mut Session,
    wait_id: &str,
    confirmation: &Confirmation,
    now: i64,
    zone: &str,
) -> Result<String> {
    vault.confirmation = Some(confirmation.clone());
    let answered =
        crate::usecases::workflow::runs::answer_workflow_wait(vault, wait_id, true, now, zone);
    vault.confirmation = None;
    let run = answered?.run;
    if let RunOutcome::Rejected { reason, .. } = run.outcome {
        return Err(rules::funds_error(&vault.doc, reason));
    }
    run.created
        .into_iter()
        .next()
        .ok_or_else(|| Error::InvalidInput("the run made no operation".into()))
}
