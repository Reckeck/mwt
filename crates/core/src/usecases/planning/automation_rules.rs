use std::collections::HashSet;

use crate::Amount;
use crate::calendar;
use crate::error::{Error, Result};
use crate::model::planning::{AutomationOutcome, AutomationRun, AutomationSpec};
use crate::model::store::Holding;
use crate::model::vault::VaultDoc;
use crate::planning::automation_plan::runs_in;
use crate::planning::automations::{
    AutomationDraft, AutomationSimulation, SimulatedBalance, SimulatedRun,
};
use crate::time::unix_secs;
use crate::workflow::automation::{self as graph_of, automation_graph};
use crate::workflow::exec::{StepStatus, TraceStep};
use crate::workflow::model::{GraphVersion, MAX_GRAPH_VERSIONS, RunOutcome, Workflow, WorkflowRun};

/// Run history over the document, for the command that holds the vault.
pub fn automation_runs(doc: &VaultDoc, automation_id: Option<&str>) -> Vec<AutomationRun> {
    runs_in(doc, automation_id)
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
    for run in runs_in(doc, None)
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
