//! What a workflow leaves in the vault: itself, its runs, its variables, the
//! operations waiting for confirmation and what its triggers last saw.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::transaction::Transaction;

/// Upper bound on stored runs over all workflows; the oldest are dropped.
pub const MAX_WORKFLOW_RUNS: usize = 1000;
/// Earlier versions of a graph kept to restore.
pub const MAX_GRAPH_VERSIONS: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub active: bool,
    /// The graph as JSON (`Graph`): nodes, edges, comments.
    pub graph: String,
    pub created_at: String,
    pub updated_at: String,
    /// Run on operations made by workflows too (off: no chains of workflows
    /// firing each other).
    #[serde(default)]
    pub react_to_workflows: bool,
    /// Most one run may move, in the base currency; `None` = no cap.
    #[serde(default)]
    pub max_amount: Option<crate::Amount>,
    /// Earlier graphs, newest last, to compare and restore.
    #[serde(default)]
    pub versions: Vec<GraphVersion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct GraphVersion {
    pub saved_at: String,
    pub graph: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    /// Ran to the end.
    Done,
    /// Stopped by a «Стоп» node or a guard, with its reason.
    Stopped { reason: String },
    /// Failed: nothing it did was kept.
    Failed { message: String },
    /// Asleep at a «Подождать» node until `until` (RFC3339).
    Waiting { until: String },
}

/// One run of a workflow, with its trace for the journal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct WorkflowRun {
    pub id: String,
    pub workflow_id: String,
    /// Trigger node id and what fired it, e.g. `schedule 2026-11-01`.
    pub trigger: String,
    pub started_at: String,
    pub outcome: RunOutcome,
    /// Operations it created.
    pub created: Vec<String>,
    /// Operations it put in the confirmation queue.
    pub queued: u32,
    /// The run in words, step by step.
    pub trace: Vec<crate::workflow::TraceStep>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VarScope {
    /// Shared by every workflow.
    Global,
    /// Kept between the runs of one workflow.
    Workflow(String),
}

/// A variable that outlives a run. The value is a `WfValue` in JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct WorkflowVar {
    pub scope: VarScope,
    pub name: String,
    pub value: String,
    pub updated_at: String,
}

/// Operations a workflow prepared behind a «Ждать подтверждения» node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct QueuedActions {
    pub id: String,
    pub workflow_id: String,
    pub run_id: String,
    pub created_at: String,
    /// What they are, in words.
    pub summary: String,
    pub transactions: Vec<Transaction>,
}

/// What a workflow's triggers last saw, so each fires once per occasion.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct WorkflowState {
    pub workflow_id: String,
    /// Trigger node id → last moment it fired (RFC3339) or last side of a
    /// threshold (`above` / `below`).
    pub marks: BTreeMap<String, String>,
    /// The last operation an «operation added» trigger looked at, and where it was.
    pub tx_cursor: Option<String>,
    pub tx_index: usize,
}

/// A run asleep at a «Подождать» node: where it goes on, when, and the data it
/// had worked out by then.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct WorkflowWait {
    pub id: String,
    pub workflow_id: String,
    pub run_id: String,
    /// The wait node; the run goes on from its «Дальше».
    pub node_id: String,
    /// RFC3339.
    pub resume_at: String,
    /// Node outputs computed before the wait, as JSON.
    pub memo: String,
    /// Whether operations after it go to the confirmation queue.
    pub queue_mode: bool,
    pub trigger: String,
}
