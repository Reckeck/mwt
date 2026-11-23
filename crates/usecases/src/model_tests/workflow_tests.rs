//! The workflow model keeps its old paths in `workflow`, a higher layer.

use crate::model::workflow::MAX_WORKFLOW_RUNS;

#[test]
fn workflow_types_keep_their_old_paths() {
    let _: usize = crate::workflow::model::MAX_WORKFLOW_RUNS;
    let _: Option<crate::workflow::model::Workflow> = None;
    let _: Option<crate::workflow::Workflow> = None;
    let _: Option<crate::workflow::model::WorkflowRun> = None;
    let _: Option<crate::workflow::exec::TraceStep> = None;
    let _: Option<crate::workflow::exec::StepStatus> = None;
    let _: Option<crate::workflow::TraceStep> = None;
    assert_eq!(MAX_WORKFLOW_RUNS, 1000);
}
