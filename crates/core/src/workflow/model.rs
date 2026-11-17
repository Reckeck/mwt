//! What a workflow leaves in the vault; the types live in [`crate::model::workflow`].

pub use crate::model::workflow::{
    GraphVersion, MAX_GRAPH_VERSIONS, MAX_WORKFLOW_RUNS, QueuedActions, RunOutcome, TestExpect,
    VarScope, WaitKind, Workflow, WorkflowRun, WorkflowState, WorkflowTest, WorkflowVar,
    WorkflowWait,
};
