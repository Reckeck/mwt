//! Node workflows (plan §16): a visual automation made of nodes joined by
//! flow (what runs, in which order) and data (values passed between nodes).
//!
//! A workflow is stored as its graph in JSON inside the vault, so new nodes and
//! parameters never need a vault format change. The catalog (`catalog.rs`)
//! describes every node — its ports, their types and its parameters — and is
//! the one source the editor draws from. `validate.rs` checks a graph before
//! it is saved, `exec.rs` runs it, and the service (`service/workflows.rs`)
//! fires the triggers, keeps the journal, the variables and the queue of
//! operations waiting for confirmation.

pub mod automation;
pub mod catalog;
mod catalog_capital;
mod catalog_more;
pub mod exec;
pub mod graph;
pub mod model;
pub mod nodes;
pub mod templates;
pub mod types;
pub mod validate;

pub use catalog::{NodeDef, ParamDef, ParamKind, PortDef, catalog, node_def};
pub use exec::{RunContext, RunMode, RunReport, TraceStep};
pub use graph::{Edge, Graph, Node, Position};
pub use model::{
    QueuedActions, RunOutcome, TestExpect, VarScope, WaitKind, Workflow, WorkflowRun,
    WorkflowState, WorkflowTest, WorkflowVar, WorkflowWait,
};
pub use types::{PortType, WfValue};
pub use validate::{GraphIssue, IssueLevel, validate};
