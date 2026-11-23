//! Shim of the `workflow` area (spec core-strangler, F2, D1): its logic moved
//! to `crate::workflow::service::{records, runs, marks, vars, tests_def}` (pure
//! functions) and `crate::usecases::workflow::{records, runs}` (scenarios). The
//! public types keep their old paths.

pub use crate::workflow::service::records::{
    ExportedStore, SavedWorkflow, WorkflowDraft, WorkflowImportPreview,
};
pub use crate::workflow::service::tests_def::WorkflowTestResult;

#[cfg(test)]
mod tests;
