//! The pure rules of the automations area that read the workflow graph: the
//! automation a workflow is, its schedule moments, the run history, the stored
//! graph, the simulation report and the record of a manual run. None takes a
//! vault or transacts. `planning` may not use `workflow` (layer table), so
//! they sit here, above both; the scenarios of `automations` call them, and
//! the budget (T41) takes the occurrences from here through its scenario.

use std::collections::HashSet;

use crate::Amount;
use crate::calendar;
use crate::calendar::Calendar;
use crate::error::{Error, Result};
use crate::model::planning::{
    AmountMode, Automation, AutomationOutcome, AutomationRun, AutomationSpec, RejectReason,
};
use crate::model::store::Holding;
use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::VaultDoc;
use crate::planning::automations::{
    self as rules, AutomationDraft, AutomationSimulation, SimulatedBalance, SimulatedRun,
    automation_draft, draft_to_transaction, main_amount, outflow,
};
use crate::pricing::history::unix_secs;
use crate::workflow::automation::{self as graph_of, automation_graph};
use crate::workflow::exec::{StepStatus, TraceStep};
use crate::workflow::model::{GraphVersion, MAX_GRAPH_VERSIONS, RunOutcome, Workflow, WorkflowRun};
use crate::workflow::nodes::triggers::schedule_moments_limited;

/// The automation a workflow is, or `None` for an ordinary workflow.
/// `recent_average` gives the average of the latest corrected amounts (asked
/// only for an amount that differs); `mark` is the stored schedule mark.
pub(crate) fn automation_of(
    workflow: &Workflow,
    recent_average: impl FnOnce(&str) -> Option<Amount>,
    mark: Option<String>,
) -> Option<Automation> {
    let spec = workflow.automation.as_ref()?;
    let estimate = (spec.amount_mode == AmountMode::Varies)
        .then(|| recent_average(&workflow.id))
        .flatten();
    let last_planned_at = mark.filter(|mark| *mark != graph_of::start_mark(spec));
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
pub(crate) fn automation_occurrences(
    automation: &Automation,
    cal: &Calendar,
    after: i64,
    until: i64,
) -> Vec<(i64, Transaction)> {
    let spec = rules::spec_of(automation);
    let start = unix_secs(&graph_of::start_mark(&spec)).unwrap_or(after);
    let (moments, _) = schedule_moments_limited(
        &graph_of::schedule_params(&spec),
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
pub(crate) fn automation_runs(doc: &VaultDoc, automation_id: Option<&str>) -> Vec<AutomationRun> {
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
pub(crate) fn automation_run_of(doc: &VaultDoc, run: &WorkflowRun) -> Option<AutomationRun> {
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

/// The time of day and the start date of a draft, checked; the start moment.
pub(crate) fn check_draft_moment(draft: &AutomationDraft) -> Result<i64> {
    if draft
        .time
        .as_deref()
        .is_some_and(|time| graph_of::parse_time(time).is_none())
    {
        return Err(Error::InvalidInput("time is HH:MM".into()));
    }
    unix_secs(&draft.starts_at).ok_or_else(|| Error::InvalidInput("choose a start date".into()))
}

/// The stored graph of an automation.
fn graph_json(spec: &AutomationSpec) -> Result<String> {
    serde_json::to_string(&automation_graph(spec))
        .map_err(|err| Error::InvalidInput(err.to_string()))
}

/// The workflow a new automation is stored as, made at `now` (RFC3339).
pub(crate) fn new_automation_workflow(
    draft: &AutomationDraft,
    id: &str,
    now: &str,
) -> Result<Workflow> {
    let spec = draft.spec();
    Ok(Workflow {
        id: id.to_string(),
        name: draft.name.trim().to_string(),
        description: String::new(),
        active: draft.active,
        graph: graph_json(&spec)?,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        react_to_workflows: false,
        max_amount: None,
        max_amount_currency: None,
        versions: Vec::new(),
        tests: Vec::new(),
        draft: None,
        automation: Some(spec),
    })
}

/// Change what an automation does: its graph is built again, the old one
/// kept as a version, at `now` (RFC3339).
pub(crate) fn rewrite_automation(
    workflow: &mut Workflow,
    draft: &AutomationDraft,
    now: &str,
) -> Result<()> {
    let spec = draft.spec();
    let graph = graph_json(&spec)?;
    if workflow.graph != graph {
        workflow.versions.push(GraphVersion {
            saved_at: now.to_string(),
            graph: std::mem::replace(&mut workflow.graph, graph),
        });
        let overflow = workflow.versions.len().saturating_sub(MAX_GRAPH_VERSIONS);
        workflow.versions.drain(..overflow);
    }
    workflow.name = draft.name.trim().to_string();
    workflow.active = draft.active;
    workflow.updated_at = now.to_string();
    workflow.automation = Some(spec);
    Ok(())
}

/// Make an automation an ordinary workflow: the graph stays and is edited
/// in the editor from now on; the form no longer shows it.
pub(crate) fn detach(workflow: &mut Workflow, now: &str) {
    workflow.automation = None;
    workflow.updated_at = now.to_string();
}

/// What the emulated days changed. `doc` is the sandbox after the days ran,
/// `known` the run ids it had before, `before` its holdings before; `now`
/// the start of the emulation and `days` its length (already clamped).
pub(crate) fn simulation_report(
    doc: &VaultDoc,
    known: &HashSet<String>,
    before: &[Holding],
    automation_id: Option<&str>,
    now: i64,
    days: u32,
) -> AutomationSimulation {
    let wanted = |id: &str| automation_id.is_none_or(|only| only == id);
    let mut out = AutomationSimulation {
        from: calendar::iso(now),
        until: calendar::iso(now + i64::from(days) * 86_400),
        ..AutomationSimulation::default()
    };
    let mut touched: HashSet<(String, String)> = HashSet::new();
    for run in automation_runs(doc, None)
        .into_iter()
        .filter(|run| !known.contains(&run.id) && wanted(&run.automation_id))
    {
        let transaction = match &run.outcome {
            AutomationOutcome::Created { tx_id } => doc
                .transactions
                .iter()
                .find(|tx| &tx.meta().id == tx_id)
                .cloned(),
            _ => None,
        };
        match &run.outcome {
            AutomationOutcome::Created { .. } => out.created += 1,
            AutomationOutcome::Rejected { .. } => out.rejected += 1,
            AutomationOutcome::Skipped => {}
        }
        if let Some(tx) = &transaction {
            for leg in tx.legs() {
                touched.insert((leg.store_id.clone(), leg.asset.key()));
            }
        }
        out.runs.push(SimulatedRun {
            automation_name: doc
                .workflows
                .iter()
                .find(|workflow| workflow.id == run.automation_id)
                .map(|workflow| workflow.name.clone())
                .unwrap_or_default(),
            automation_id: run.automation_id,
            planned_at: run.planned_at,
            outcome: run.outcome,
            transaction,
        });
    }
    out.runs
        .sort_by(|left, right| left.planned_at.cmp(&right.planned_at));
    let quantity = |holdings: &[Holding], key: &(String, String)| {
        holdings
            .iter()
            .find(|holding| holding.store_id == key.0 && holding.asset.key() == key.1)
            .map(|holding| (holding.asset.clone(), holding.quantity))
    };
    let mut balances: Vec<SimulatedBalance> = touched
        .iter()
        .filter_map(|key| {
            let after = quantity(&doc.holdings, key);
            let was = quantity(before, key);
            let asset = after.as_ref().or(was.as_ref())?.0.clone();
            Some(SimulatedBalance {
                store_id: key.0.clone(),
                asset,
                before: was.map_or(Amount::ZERO, |(_, value)| value),
                after: after.map_or(Amount::ZERO, |(_, value)| value),
            })
        })
        .collect();
    balances.sort_by(|left, right| {
        (&left.store_id, &left.asset.symbol).cmp(&(&right.store_id, &right.asset.symbol))
    });
    out.balances = balances;
    out
}

/// The history record of a run made by hand that booked `tx_id`.
pub(crate) fn manual_run_record(
    run_id: &str,
    automation_id: &str,
    planned_at: &str,
    tx_id: &str,
) -> WorkflowRun {
    WorkflowRun {
        id: run_id.to_string(),
        workflow_id: automation_id.to_string(),
        trigger: format!("manual {planned_at}"),
        started_at: planned_at.to_string(),
        outcome: RunOutcome::Done,
        created: vec![tx_id.to_string()],
        queued: 0,
        trace: vec![TraceStep {
            node_id: graph_of::ACTION.into(),
            kind: "action.transaction".into(),
            status: StepStatus::Ok,
            text_ru: "Создано вручную: «Создать сейчас»".into(),
            text_en: "Made by hand: «Create now»".into(),
            values: Default::default(),
        }],
        dry_run: false,
        planned_date: None,
    }
}
