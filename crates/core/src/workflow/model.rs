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
    /// Checks run by emulation: what the workflow should do.
    #[serde(default)]
    pub tests: Vec<WorkflowTest>,
    /// Edits left unsaved in the editor, kept to come back to; the saved
    /// graph (and the runs) are not touched by them.
    #[serde(default)]
    #[ts(optional)]
    pub draft: Option<GraphVersion>,
    /// Set on a workflow made by the automation form: its graph is built
    /// from this and edited there, not in the editor.
    #[serde(default)]
    #[ts(optional)]
    pub automation: Option<crate::model::planning::AutomationSpec>,
}

/// An expectation checked by emulation: run the workflow as at a moment and
/// compare what it did with what it should do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct WorkflowTest {
    pub id: String,
    pub name: String,
    /// The trigger to start from; a manual one, else the first, by default.
    #[serde(default)]
    pub trigger_node: Option<String>,
    /// The moment to emulate (RFC3339); now when empty.
    #[serde(default)]
    pub at: Option<String>,
    /// Answers to "Wait for confirmation" questions, by node id; a question
    /// without one ends the emulation as `waiting`.
    #[serde(default)]
    #[ts(optional)]
    pub answers: Option<BTreeMap<String, bool>>,
    pub expect: TestExpect,
}

/// What a test expects; `None` = not checked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TestExpect {
    /// `done`, `stopped`, `failed` or `waiting`.
    pub outcome: String,
    #[serde(default)]
    pub transactions: Option<u32>,
    #[serde(default)]
    pub queued: Option<u32>,
    #[serde(default)]
    pub notices: Option<u32>,
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
    /// Stopped by a "Stop" node or a guard, with its reason.
    Stopped { reason: String },
    /// Failed: nothing it did was kept.
    Failed { message: String },
    /// Asleep at a "Wait" node until `until` (RFC3339).
    Waiting { until: String },
    /// Asleep at a "Wait for confirmation" node until the user answers (or,
    /// with a time limit, until `until`, which counts as «No»).
    Asking {
        node_id: String,
        question: String,
        #[serde(default)]
        until: Option<String>,
    },
    /// Ran to the end, but some branches failed: each failed branch was
    /// undone, the rest kept. One line per failed branch.
    Partial { failed: Vec<String> },
    /// Refused for want of money at a "Create transaction" set to refuse:
    /// nothing was kept; the run waits for "Retry" or "Skip".
    /// `trigger_node` and `planned_at` let a retry fire it the same way.
    Rejected {
        reason: crate::model::planning::RejectReason,
        trigger_node: String,
        #[serde(default)]
        planned_at: Option<String>,
    },
    /// Never ran: `off` (the workflow was switched off while a run slept),
    /// `node_removed` (its wait node is gone), `rate_limit` (too many runs
    /// this hour) or `skipped` (a refused run the user skipped, or a planned
    /// moment beyond the catch-up limit).
    Dropped { reason: String },
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

/// Operations a workflow prepared behind a "Wait for confirmation" node.
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
    #[serde(default)]
    pub marks: BTreeMap<String, String>,
    /// The last operation an «operation added» trigger looked at, and where it was.
    /// Device-local since sync: never merged, see `sync::registry`.
    #[serde(default)]
    pub tx_cursor: Option<String>,
    #[serde(default)]
    pub tx_index: usize,
}

/// What a sleeping run waits for.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WaitKind {
    /// "Wait": goes on from "Next" at `resume_at`.
    #[default]
    Timer,
    /// "Wait for confirmation": goes on from "Yes" or "No" when the user
    /// answers; `resume_at`, when set, is the deadline that counts as "No".
    Confirm,
}

/// A run asleep at a "Wait" or "Wait for confirmation" node: where it goes
/// on, when, and the data it had worked out by then.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct WorkflowWait {
    pub id: String,
    pub workflow_id: String,
    pub run_id: String,
    /// The wait node; the run goes on from its "Next" (or "Yes"/"No").
    pub node_id: String,
    /// RFC3339; empty for a question without a deadline.
    pub resume_at: String,
    /// Node outputs computed before the wait, as JSON.
    pub memo: String,
    /// Unused: the confirmation queue is gone (a "Wait for confirmation" node
    /// asks instead). Kept so older vaults read.
    pub queue_mode: bool,
    pub trigger: String,
    #[serde(default)]
    pub kind: WaitKind,
    /// The question of a "Wait for confirmation" node.
    #[serde(default)]
    pub question: String,
    /// When the run fell asleep (RFC3339).
    #[serde(default)]
    pub created_at: String,
    /// The graph as it was when the run fell asleep, as JSON: the run goes
    /// on along it even if the workflow was edited since. Empty for a wait
    /// from an older app, which goes on along the current graph.
    #[serde(default)]
    #[ts(skip)]
    pub graph: String,
}
