//! The automation plan reads: the automation a workflow is, its schedule
//! moments, the run history and the list of automations. They read the
//! books (the document), never transact; the scenarios of
//! `usecases::planning::automations` and the budget call them directly.

use std::collections::HashSet;

use crate::automations::{self as rules, automation_draft, main_amount, outflow};
use mwt_base::Amount;
use mwt_base::calendar;
use mwt_base::calendar::Calendar;
use mwt_base::time::unix_secs;
use mwt_model::VaultDoc;
use mwt_model::{
    AmountMode, Automation, AutomationOutcome, AutomationRun, RejectReason, draft_to_transaction,
};
use mwt_model::{
    ESTIMATE_OF, RunOutcome, Workflow, WorkflowRun, recent_average, schedule_key,
    schedule_moments_limited, schedule_params, start_mark, workflow_mark,
};
use mwt_model::{Transaction, TxOrigin};
use mwt_money::books::Books;

/// The automation a workflow is in the books, or `None` for an ordinary
/// workflow: the estimate and the schedule mark come from the document.
pub fn automation_in(books: &Books<'_>, workflow: &Workflow) -> Option<Automation> {
    let doc = books.doc();
    automation_of(
        workflow,
        |id| recent_average(doc, id, ESTIMATE_OF).map(|(average, _)| average),
        workflow_mark(doc, &workflow.id, &schedule_key()),
    )
}

/// Every automation, by name.
pub fn automations(books: &Books<'_>) -> Vec<Automation> {
    let mut out: Vec<Automation> = books
        .doc()
        .workflows
        .iter()
        .filter_map(|workflow| automation_in(books, workflow))
        .collect();
    out.sort_by_key(|automation| automation.name.to_lowercase());
    out
}

/// The automation a workflow is, or `None` for an ordinary workflow.
/// `recent_average` gives the average of the latest corrected amounts (asked
/// only for an amount that differs); `mark` is the stored schedule mark.
pub fn automation_of(
    workflow: &Workflow,
    recent_average: impl FnOnce(&str) -> Option<Amount>,
    mark: Option<String>,
) -> Option<Automation> {
    let spec = workflow.automation.as_ref()?;
    let estimate = (spec.amount_mode == AmountMode::Varies)
        .then(|| recent_average(&workflow.id))
        .flatten();
    let last_planned_at = mark.filter(|mark| *mark != start_mark(spec));
    Some(Automation {
        id: workflow.id.clone(),
        name: workflow.name.clone(),
        active: workflow.active,
        preset: spec.preset,
        schedule: spec.schedule,
        starts_at: spec.starts_at.clone(),
        template: spec.template.clone(),
        last_planned_at,
        created_at: workflow.created_at.clone(),
        amount_mode: spec.amount_mode,
        workday_shift: spec.workday_shift,
        estimate,
        property_id: spec.property_id.clone(),
        time: spec.time.clone(),
        asks: spec.asks,
        for_month_offset: spec.for_month_offset,
    })
}

/// The planned moments of an automation in `(after, until]`, moved off
/// days off as it is set to, each with the operation it would make: what
/// the forecast, the calendar and the daily budget count on.
pub fn automation_occurrences(
    automation: &Automation,
    cal: &Calendar,
    after: i64,
    until: i64,
) -> Vec<(i64, Transaction)> {
    let spec = rules::spec_of(automation);
    let start = unix_secs(&start_mark(&spec)).unwrap_or(after);
    let (moments, _) = schedule_moments_limited(
        &schedule_params(&spec),
        cal,
        after.max(start),
        until,
        usize::MAX,
    );
    let draft = automation_draft(automation);
    moments
        .into_iter()
        .map(|moment| {
            let iso = calendar::iso(moment);
            let mut tx = draft_to_transaction(&draft, &automation.id, &iso, &iso);
            if let TxOrigin::Workflow { estimated, .. } = &mut tx.meta_mut().origin {
                *estimated = automation.amount_mode == AmountMode::Varies;
            }
            (moment, tx)
        })
        .collect()
}

/// Run history, newest planned date first.
pub fn automation_runs(books: &Books<'_>, automation_id: Option<&str>) -> Vec<AutomationRun> {
    runs_in(books.doc(), automation_id)
}

/// [`automation_runs`] over a bare document (a sandbox the emulation ran in).
pub fn runs_in(doc: &VaultDoc, automation_id: Option<&str>) -> Vec<AutomationRun> {
    let ids: HashSet<&str> = doc
        .workflows
        .iter()
        .filter(|workflow| workflow.automation.is_some())
        .map(|workflow| workflow.id.as_str())
        .filter(|id| automation_id.is_none_or(|only| only == *id))
        .collect();
    let mut out: Vec<AutomationRun> = doc
        .workflow_runs
        .iter()
        .filter(|run| !run.dry_run && ids.contains(run.workflow_id.as_str()))
        .filter_map(|run| automation_run_of(doc, run))
        .collect();
    out.sort_by(|left, right| right.planned_at.cmp(&left.planned_at));
    out
}

/// A workflow run as the automation history shows it; `None` for a run the
/// history does not list.
pub fn automation_run_of(doc: &VaultDoc, run: &WorkflowRun) -> Option<AutomationRun> {
    let planned_at = match &run.outcome {
        RunOutcome::Rejected {
            planned_at: Some(at),
            ..
        } => at.clone(),
        _ => run
            .trigger
            .strip_prefix("schedule ")
            .map_or_else(|| run.started_at.clone(), str::to_string),
    };
    let outcome = match &run.outcome {
        RunOutcome::Done | RunOutcome::Partial { .. } => AutomationOutcome::Created {
            tx_id: run.created.first()?.clone(),
        },
        RunOutcome::Rejected { reason, .. } => AutomationOutcome::Rejected {
            reason: reason.clone(),
        },
        RunOutcome::Failed { message } => AutomationOutcome::Rejected {
            reason: RejectReason::Invalid {
                message: message.clone(),
            },
        },
        RunOutcome::Dropped { reason } if reason == "skipped" => AutomationOutcome::Skipped,
        _ => return None,
    };
    let made = match &outcome {
        AutomationOutcome::Created { tx_id } => {
            doc.transactions.iter().find(|tx| &tx.meta().id == tx_id)
        }
        _ => None,
    };
    let estimated = made.is_some_and(|tx| tx.meta().origin.is_estimated());
    let amount = made.and_then(main_amount);
    let symbol = made
        .and_then(|tx| {
            outflow(tx).map(|flow| flow.asset.symbol).or_else(|| {
                tx.legs()
                    .into_iter()
                    .find(|leg| leg.amount.is_positive())
                    .map(|leg| leg.asset.symbol)
            })
        })
        .unwrap_or_default();
    Some(AutomationRun {
        id: run.id.clone(),
        automation_id: run.workflow_id.clone(),
        planned_at,
        processed_at: run.started_at.clone(),
        outcome,
        estimated,
        amount,
        symbol,
    })
}
