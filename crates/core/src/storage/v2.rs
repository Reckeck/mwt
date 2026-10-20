//! Payload v2, frozen: what changed in v3 is copied here as it was.
//!
//! v3 runs automations as workflows: the `automations` and `automation_runs`
//! collections are gone, each automation became a workflow built from its
//! form (`Workflow::automation`), keeping its id, and each run a workflow run.
//! Everything else of the document is read with the current types (v2 files
//! carry field names, so a later field with a default reads fine).
//!
//! Never edit these definitions: they describe files already on users' disks.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::planning::{
    AmountMode, AutomationOutcome, AutomationPreset, AutomationSpec, RejectReason, Schedule,
    TransactionDraft, WorkdayShift,
};
use crate::model::vault::VaultDoc;
use crate::workflow::exec::{StepStatus, TraceStep};
use crate::workflow::model::{RunOutcome, Workflow, WorkflowRun, WorkflowState};

/// An automation as v1 and v2 stored it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct AutomationV2 {
    pub id: String,
    pub name: String,
    pub active: bool,
    #[serde(default)]
    pub preset: Option<AutomationPreset>,
    pub schedule: Schedule,
    pub starts_at: String,
    pub template: TransactionDraft,
    pub last_planned_at: Option<String>,
    pub created_at: String,
}

/// A processed planned date as v1 and v2 stored it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct AutomationRunV2 {
    pub id: String,
    pub automation_id: String,
    pub planned_at: String,
    pub processed_at: String,
    pub outcome: AutomationOutcome,
}

/// The v2 document: today's one plus the two automation collections.
#[derive(Deserialize)]
pub struct VaultDocV2 {
    #[serde(flatten)]
    pub doc: VaultDoc,
    #[serde(default)]
    pub automations: Vec<AutomationV2>,
    #[serde(default)]
    pub automation_runs: Vec<AutomationRunV2>,
}

/// v2 → v3: each automation becomes a workflow with the same id, so the
/// operations it made (`TxOrigin::Automation`) still point at it; its last
/// processed date becomes the schedule's mark, so nothing runs twice; each
/// run becomes a workflow run with the same outcome.
impl From<VaultDocV2> for VaultDoc {
    fn from(old: VaultDocV2) -> Self {
        let mut doc = old.doc;
        doc.version = 3;
        for automation in old.automations {
            let spec = AutomationSpec {
                preset: automation.preset,
                schedule: automation.schedule,
                starts_at: automation.starts_at.clone(),
                template: automation.template,
                amount_mode: AmountMode::Fixed,
                workday_shift: WorkdayShift::None,
            };
            let graph = crate::workflow::automation::automation_graph(&spec);
            let mark = automation
                .last_planned_at
                .clone()
                .unwrap_or_else(|| crate::workflow::automation::start_mark(&spec));
            doc.workflow_state.push(WorkflowState {
                workflow_id: automation.id.clone(),
                marks: BTreeMap::from([(crate::workflow::automation::schedule_key(), mark)]),
                tx_cursor: doc.transactions.last().map(|tx| tx.meta().id.clone()),
                tx_index: doc.transactions.len(),
            });
            doc.workflows.push(Workflow {
                id: automation.id,
                name: automation.name,
                description: String::new(),
                active: automation.active,
                graph: serde_json::to_string(&graph).unwrap_or_default(),
                created_at: automation.created_at.clone(),
                updated_at: automation.created_at,
                react_to_workflows: false,
                max_amount: None,
                versions: Vec::new(),
                tests: Vec::new(),
                draft: None,
                automation: Some(spec),
            });
        }
        let mut runs: Vec<WorkflowRun> = old.automation_runs.into_iter().map(run_of).collect();
        runs.extend(std::mem::take(&mut doc.workflow_runs));
        runs.sort_by(|left, right| left.started_at.cmp(&right.started_at));
        let overflow = runs
            .len()
            .saturating_sub(crate::workflow::model::MAX_WORKFLOW_RUNS);
        runs.drain(..overflow);
        doc.workflow_runs = runs;
        doc
    }
}

fn run_of(run: AutomationRunV2) -> WorkflowRun {
    let (outcome, created, status) = match run.outcome {
        AutomationOutcome::Created { tx_id } => (RunOutcome::Done, vec![tx_id], StepStatus::Ok),
        AutomationOutcome::Rejected { reason } => (
            RunOutcome::Rejected {
                reason,
                trigger_node: crate::workflow::automation::TRIGGER.into(),
                planned_at: Some(run.planned_at.clone()),
            },
            Vec::new(),
            StepStatus::Error,
        ),
        AutomationOutcome::Skipped => (
            RunOutcome::Dropped {
                reason: "skipped".into(),
            },
            Vec::new(),
            StepStatus::Skipped,
        ),
    };
    let text = match &outcome {
        RunOutcome::Rejected {
            reason: RejectReason::Invalid { message },
            ..
        } => message.clone(),
        RunOutcome::Rejected { reason, .. } => crate::service::describe_reject(reason),
        _ => String::new(),
    };
    WorkflowRun {
        id: run.id,
        workflow_id: run.automation_id,
        trigger: format!("schedule {}", run.planned_at),
        started_at: run.processed_at,
        outcome,
        created,
        queued: 0,
        trace: vec![TraceStep {
            node_id: crate::workflow::automation::ACTION.into(),
            kind: "action.transaction".into(),
            status,
            text_ru: text.clone(),
            text_en: text,
            values: Default::default(),
        }],
        dry_run: false,
    }
}
