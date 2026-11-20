//! Runs: the journal, the queue and the waits read as they are, which
//! occasions are already booked, what a trigger would hand on, and the
//! journal entries made for a run that never happened. Pure functions over
//! the document or the host; the scenarios in `usecases::workflow::runs` write.

use std::collections::BTreeMap;

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::{Error, Result};
use crate::model::notify::NoticeRecord;
use crate::model::store::AssetRef;
use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::VaultDoc;
use crate::pricing::history::unix_secs;
use crate::time::LocalDate;
use crate::workflow::catalog::node_def;
use crate::workflow::exec::{RunContext, RunReport, StepStatus, TraceStep};
use crate::workflow::graph::{Graph, Node};
use crate::workflow::host::WorkflowHost;
use crate::workflow::model::{QueuedActions, RunOutcome, Workflow, WorkflowRun, WorkflowWait};
use crate::workflow::nodes::triggers::{schedule_moments, tx_matches, tx_trigger_values};
use crate::workflow::types::WfValue;

/// Most live runs of one workflow in an hour; more look like a loop.
pub const MAX_RUNS_PER_HOUR: usize = 60;
/// How deep events may chain one workflow into another.
pub const MAX_EVENT_DEPTH: u32 = 5;

/// A planned moment an operation or a run booked: `(workflow id, stored local
/// date, RFC3339 moment)`.
pub type Booked = (String, Option<LocalDate>, String);

/// What one scheduler pass did.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WorkflowPass {
    pub runs: Vec<WorkflowRun>,
    /// Operations made.
    pub created: Vec<String>,
    /// Operations put into the confirmation queue.
    pub queued: u32,
    /// New notices to show.
    pub notices: Vec<NoticeRecord>,
    /// Planned moments of automations beyond the catch-up limit, recorded as
    /// skipped.
    #[serde(default)]
    pub skipped: usize,
}

impl WorkflowPass {
    /// Take in what a live run left in the pass: the operations, the queue,
    /// the notices and the run itself.
    pub fn absorb(&mut self, report: &RunReport) {
        self.created.extend(report.run.created.iter().cloned());
        self.queued += report.run.queued;
        self.notices.extend(report.notices.iter().cloned());
        self.runs.push(report.run.clone());
    }
}

/// Whether a workflow (or the automation it was migrated from, which kept its
/// id) made the operation.
pub(crate) fn made_by(tx: &Transaction, workflow_id: &str) -> bool {
    match &tx.meta().origin {
        TxOrigin::Workflow {
            workflow_id: maker, ..
        }
        | TxOrigin::Automation {
            automation_id: maker,
            ..
        } => maker == workflow_id,
        _ => false,
    }
}

/// What fired a run, for the journal: `schedule 2026-11-01T09:00:00Z`.
pub fn trigger_label(kind: &str, detail: &str) -> String {
    let short = kind.strip_prefix("trigger.").unwrap_or(kind);
    if detail.is_empty() {
        short.to_string()
    } else {
        format!("{short} {detail}")
    }
}

/// Whether a workflow gathers the firings of a pass: it has a "Queue" that is on.
pub fn has_queue(graph: &Graph) -> bool {
    graph
        .nodes
        .iter()
        .any(|node| node.kind == "flow.queue" && !node.disabled)
}

/// The graph a sleeping run goes on along: the one it fell asleep on, or the
/// workflow's current one for a wait from an older app.
pub fn wait_graph(wait: &WorkflowWait, workflow: &Workflow) -> Result<Graph> {
    if wait.graph.trim().is_empty() {
        Graph::parse(&workflow.graph)
    } else {
        Graph::parse(&wait.graph)
    }
}

/// The trigger to start from: the one named, else a manual one, else the first.
pub fn pick_trigger(graph: &Graph, wanted: Option<&str>) -> Result<Node> {
    let triggers: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| node_def(&node.kind).is_some_and(|def| def.is_trigger()))
        .collect();
    if let Some(id) = wanted {
        return triggers
            .into_iter()
            .find(|node| node.id == id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("trigger {id}")));
    }
    triggers
        .iter()
        .find(|node| node.kind == "trigger.manual")
        .or_else(|| triggers.first())
        .map(|node| (*node).clone())
        .ok_or_else(|| Error::InvalidInput("the workflow has no trigger".into()))
}

/// The run context of one firing of `trigger`, with the cap of its workflow.
pub fn context(workflow: &Workflow, trigger: &Node, now: i64, zone: &str) -> RunContext {
    let mut ctx = RunContext::new(&workflow.id, &workflow.name, &trigger.id, now, zone);
    ctx.set_cap(workflow);
    ctx
}

// -- the journal, the queue, the waits --------------------------------------------

/// Runs, newest first, of one workflow or of all.
pub fn runs(doc: &VaultDoc, workflow_id: Option<&str>, limit: usize) -> Vec<WorkflowRun> {
    doc.workflow_runs
        .iter()
        .rev()
        .filter(|run| workflow_id.is_none_or(|id| run.workflow_id == id))
        .take(limit)
        .cloned()
        .collect()
}

pub fn queue(doc: &VaultDoc) -> Vec<QueuedActions> {
    doc.workflow_queue.clone()
}

/// Runs asleep: timers and questions waiting for an answer, oldest first.
pub fn waits(doc: &VaultDoc) -> Vec<WorkflowWait> {
    doc.workflow_waits.clone()
}

pub fn active_workflows(doc: &VaultDoc) -> Vec<Workflow> {
    doc.workflows
        .iter()
        .filter(|workflow| workflow.active)
        .cloned()
        .collect()
}

pub fn runs_last_hour(doc: &VaultDoc, workflow_id: &str, now: i64) -> usize {
    doc.workflow_runs
        .iter()
        .filter(|run| run.workflow_id == workflow_id && !run.dry_run)
        .filter(|run| !matches!(run.outcome, RunOutcome::Dropped { .. }))
        .filter(|run| unix_secs(&run.started_at).is_some_and(|secs| now - secs < 3600))
        .count()
}

/// A wait id this vault has neither got nor deleted: a wait that was resumed
/// (its removal left a tombstone) is not asked again, and one that is already
/// here is not doubled.
pub fn wait_is_new(doc: &VaultDoc, wait_id: &str) -> bool {
    doc.workflow_waits.iter().all(|known| known.id != wait_id)
        && !doc
            .sync
            .tombstones
            .contains_key(&format!("workflow_waits:{wait_id}"))
}

// -- which occasions are booked -------------------------------------------------

/// Planned moments operations were booked for, as `(workflow id, stored
/// planned date, RFC3339 moment)`: the same marker the scheduled run itself
/// goes by. The one place that reads the origin of an operation.
pub fn booked_moments(doc: &VaultDoc) -> Vec<Booked> {
    let mut out = Vec::new();
    for tx in &doc.transactions {
        match &tx.meta().origin {
            TxOrigin::Automation {
                automation_id,
                planned_at,
            } => out.push((automation_id.clone(), None, planned_at.clone())),
            TxOrigin::Workflow {
                workflow_id,
                planned_at,
                planned_date,
                ..
            } if planned_at.is_some() || planned_date.is_some() => out.push((
                workflow_id.clone(),
                *planned_date,
                planned_at.clone().unwrap_or_default(),
            )),
            _ => {}
        }
    }
    out
}

/// Planned moments of automations already processed, by the operation they
/// made or by a run: `(workflow id, stored planned date, RFC3339 moment)`.
/// Guards against a second booking even when a schedule mark was rolled back
/// (a sync). Only records without a stored date (old ones) are dated later by
/// their moment.
pub fn automation_processed(doc: &VaultDoc) -> Vec<Booked> {
    let mut out = booked_moments(doc);
    for run in &doc.workflow_runs {
        let planned = match &run.outcome {
            RunOutcome::Rejected {
                planned_at: Some(at),
                ..
            } => Some(at.clone()),
            _ => run.trigger.strip_prefix("schedule ").map(str::to_string),
        };
        if planned.is_some() || run.planned_date.is_some() {
            out.push((
                run.workflow_id.clone(),
                run.planned_date,
                planned.unwrap_or_default(),
            ));
        }
    }
    out
}

/// Whether an operation already books `moment` of `workflow_id`.
pub fn moment_booked(doc: &VaultDoc, workflow_id: &str, moment: &str) -> bool {
    booked_moments(doc)
        .iter()
        .any(|(id, _, booked)| id == workflow_id && booked == moment)
}

/// Whether an operation already carries the run key `key`.
pub fn run_key_booked(doc: &VaultDoc, key: &str) -> bool {
    doc.transactions.iter().any(|tx| {
        matches!(&tx.meta().origin, TxOrigin::Workflow { run_key: Some(known), .. } if known == key)
    })
}

/// Whether an operation from before run keys books `moment` of `workflow_id`
/// (it names the moment but no key).
pub fn legacy_moment_booked(doc: &VaultDoc, workflow_id: &str, moment: &str) -> bool {
    doc.transactions.iter().any(|tx| match &tx.meta().origin {
        TxOrigin::Workflow {
            workflow_id: maker,
            planned_at: Some(planned),
            run_key: None,
            ..
        } => maker == workflow_id && planned == moment,
        TxOrigin::Automation {
            automation_id,
            planned_at,
        } => automation_id == workflow_id && planned_at == moment,
        _ => false,
    })
}

/// Whether a run of this firing's key already happened here.
pub fn already_run(doc: &VaultDoc, ctx: &RunContext) -> bool {
    ctx.run_key
        .as_deref()
        .is_some_and(|key| run_key_booked(doc, key))
}

/// Record the local date a run was planned for on the operations it made.
pub fn stamp_planned_date(doc: &mut VaultDoc, created: &[String], date: LocalDate) {
    for tx in &mut doc.transactions {
        if !created.contains(&tx.meta().id) {
            continue;
        }
        if let TxOrigin::Workflow { planned_date, .. } = &mut tx.meta_mut().origin {
            *planned_date = Some(date);
        }
    }
}

// -- what a trigger would hand on --------------------------------------------------

/// What a source trigger hands on: the source's latest value.
pub fn source_trigger_values(
    host: &dyn WorkflowHost,
    trigger: &Node,
) -> Option<BTreeMap<String, WfValue>> {
    let id = trigger.params.get("source")?.as_str()?;
    let source = host.data_source(id)?;
    let last = source.last.as_ref()?;
    Some(BTreeMap::from([
        (
            "value".to_string(),
            last.number.map_or(WfValue::Null, WfValue::Number),
        ),
        (
            "text".to_string(),
            last.text.clone().map_or(WfValue::Null, WfValue::Text),
        ),
    ]))
}

/// The watched value of a threshold trigger and which side of it it is on.
pub fn threshold_value(
    host: &dyn WorkflowHost,
    trigger: &Node,
    now: i64,
) -> Option<(WfValue, bool)> {
    let params = &trigger.params;
    let threshold = params
        .get("value")
        .and_then(crate::workflow::nodes::json_number)
        .unwrap_or(Amount::ZERO);
    let asset = params
        .get("asset")
        .filter(|value| !value.is_null())
        .and_then(|value| match value {
            serde_json::Value::String(symbol) => Some(host.wf_asset(symbol)),
            other => serde_json::from_value::<AssetRef>(other.clone()).ok(),
        });
    let value = if trigger.kind == "trigger.price" {
        let currency = params
            .get("currency")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("usd")
            .to_lowercase();
        let price = host.wf_price(asset.as_ref()?, &currency, now)?;
        WfValue::money(price, &currency)
    } else {
        let store = params
            .get("store")
            .and_then(crate::workflow::nodes::json_account)?;
        host.wf_balance(store.store_id()?, asset.as_ref()).ok()?
    };
    let above = value.as_number()? >= threshold;
    Some((value, above))
}

/// What a trigger would hand on now: for an operation trigger the latest
/// matching operation, for thresholds the current value.
pub fn sample_trigger_values(
    host: &dyn WorkflowHost,
    transactions: &[Transaction],
    trigger: &Node,
    now: i64,
    zone: &str,
) -> BTreeMap<String, WfValue> {
    let at = || BTreeMap::from([("at".to_string(), WfValue::Date(crate::calendar::iso(now)))]);
    match trigger.kind.as_str() {
        // As if the schedule fired at its latest planned moment.
        "trigger.schedule" => {
            let (moments, _) = schedule_moments(&trigger.params, &Calendar::new(zone), 0, now);
            let moment = moments.last().copied().unwrap_or(now);
            BTreeMap::from([(
                "at".to_string(),
                WfValue::Date(crate::calendar::iso(moment)),
            )])
        }
        "trigger.transaction" => transactions
            .iter()
            .rev()
            .find(|tx| tx_matches(host, &trigger.params, tx))
            .map(tx_trigger_values)
            .unwrap_or_default(),
        "trigger.balance" => threshold_value(host, trigger, now)
            .map(|(value, _)| BTreeMap::from([("balance".to_string(), value)]))
            .unwrap_or_default(),
        "trigger.price" => threshold_value(host, trigger, now)
            .map(|(value, _)| BTreeMap::from([("price".to_string(), value)]))
            .unwrap_or_default(),
        "trigger.source" => source_trigger_values(host, trigger).unwrap_or_default(),
        _ => at(),
    }
}

/// An example batch for "Queue" when run by hand or emulated: the latest
/// firings of an operation trigger (up to 3, oldest first).
pub fn sample_batch(
    host: &dyn WorkflowHost,
    transactions: &[Transaction],
    graph: &Graph,
    trigger: &Node,
    now: i64,
    zone: &str,
) -> Vec<BTreeMap<String, WfValue>> {
    if !has_queue(graph) {
        return Vec::new();
    }
    if trigger.kind == "trigger.transaction" {
        let mut latest: Vec<_> = transactions
            .iter()
            .rev()
            .filter(|tx| tx_matches(host, &trigger.params, tx))
            .take(3)
            .map(tx_trigger_values)
            .collect();
        latest.reverse();
        if !latest.is_empty() {
            return latest;
        }
    }
    vec![sample_trigger_values(
        host,
        transactions,
        trigger,
        now,
        zone,
    )]
}

// -- journal entries of runs that did not happen --------------------------------

/// A run that never happened, as a journal entry with its reason, so nothing
/// is lost without a trace.
pub fn dropped_run(
    workflow_id: &str,
    trigger: &str,
    reason: &str,
    now: i64,
    id: String,
) -> WorkflowRun {
    let (ru, en) = match reason {
        "off" => (
            "Схема выключена, пока запуск ждал: продолжения не будет",
            "The workflow was switched off while the run slept: it does not go on",
        ),
        "node_removed" => (
            "Узел ожидания удалён из схемы: продолжения не будет",
            "The wait node was removed from the workflow: the run does not go on",
        ),
        "skipped" => (
            "Пропущено: запуск не выполнялся",
            "Skipped: the run was not made",
        ),
        _ => (
            "Больше 60 запусков за час: этот пропущен",
            "More than 60 runs this hour: this one is skipped",
        ),
    };
    dropped_entry(
        id,
        workflow_id,
        trigger,
        reason,
        now,
        String::new(),
        ru.into(),
        en.into(),
    )
}

/// The journal entry of a workflow switched off on open for a required field
/// left empty. One record per workflow, whatever the device: every device that
/// opens the vault writes the same id, so the copies merge into one.
pub fn switched_off_run(
    workflow_id: &str,
    node_id: String,
    text_ru: &str,
    text_en: &str,
    now: i64,
) -> WorkflowRun {
    dropped_entry(
        format!("wf-off-{workflow_id}"),
        workflow_id,
        "",
        "missing_params",
        now,
        node_id,
        format!("Схема выключена: {text_ru}"),
        format!("The workflow was switched off: {text_en}"),
    )
}

#[allow(clippy::too_many_arguments)]
fn dropped_entry(
    id: String,
    workflow_id: &str,
    trigger: &str,
    reason: &str,
    now: i64,
    node_id: String,
    text_ru: String,
    text_en: String,
) -> WorkflowRun {
    WorkflowRun {
        id,
        workflow_id: workflow_id.to_string(),
        trigger: trigger.to_string(),
        started_at: crate::calendar::iso(now),
        outcome: RunOutcome::Dropped {
            reason: reason.to_string(),
        },
        created: Vec::new(),
        queued: 0,
        trace: vec![TraceStep {
            node_id,
            kind: String::new(),
            status: StepStatus::Skipped,
            text_ru,
            text_en,
            values: Default::default(),
        }],
        dry_run: false,
        planned_date: None,
    }
}
