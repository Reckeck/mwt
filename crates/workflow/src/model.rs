//! What a workflow leaves in the vault; the types live in `mwt-model`.

pub use mwt_model::{
    GraphVersion, MAX_GRAPH_VERSIONS, MAX_WORKFLOW_RUNS, QueuedActions, RunOutcome, TestExpect,
    VarScope, WaitKind, Workflow, WorkflowRun, WorkflowState, WorkflowTest, WorkflowVar,
    WorkflowWait,
};
