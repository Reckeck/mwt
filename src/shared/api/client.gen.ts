// AUTO-GENERATED from the #[ipc] command metadata. Do not edit manually.
// Regenerate: UPDATE_CLIENT=1 cargo test -p trade-app client_gen
import { call } from './ipc'

export function listWorkflows(): Promise<Array<Workflow>> {
  return call<Array<Workflow>>('list_workflows')
}

export function getWorkflow(id: string): Promise<Workflow> {
  return call<Workflow>('get_workflow', { id })
}

export function saveWorkflow(input: SaveWorkflowInput): Promise<SavedWorkflow> {
  return call<SavedWorkflow>('save_workflow', { input })
}

export function saveWorkflowDraft(input: SaveWorkflowDraftInput): Promise<Workflow> {
  return call<Workflow>('save_workflow_draft', { input })
}

export function discardWorkflowDraft(id: string): Promise<Workflow> {
  return call<Workflow>('discard_workflow_draft', { id })
}

export function setWorkflowActive(input: SetWorkflowActiveInput): Promise<Workflow> {
  return call<Workflow>('set_workflow_active', { input })
}

export function deleteWorkflow(id: string): Promise<null> {
  return call<null>('delete_workflow', { id })
}

export function restoreWorkflowVersion(input: RestoreWorkflowVersionInput): Promise<SavedWorkflow> {
  return call<SavedWorkflow>('restore_workflow_version', { input })
}

export function dryRunWorkflow(input: DryRunWorkflowInput): Promise<RunReport> {
  return call<RunReport>('dry_run_workflow', { input })
}

export function listWorkflowRuns(input: ListWorkflowRunsInput): Promise<Array<WorkflowRun>> {
  return call<Array<WorkflowRun>>('list_workflow_runs', { input })
}

export function listWorkflowQueue(): Promise<Array<QueuedActions>> {
  return call<Array<QueuedActions>>('list_workflow_queue')
}

export function discardWorkflowQueued(id: string): Promise<null> {
  return call<null>('discard_workflow_queued', { id })
}

export function listWorkflowWaits(): Promise<Array<WorkflowWait>> {
  return call<Array<WorkflowWait>>('list_workflow_waits')
}

export function listWorkflowVars(): Promise<Array<WorkflowVar>> {
  return call<Array<WorkflowVar>>('list_workflow_vars')
}

export function setWorkflowVar(input: SetWorkflowVarInput): Promise<null> {
  return call<null>('set_workflow_var', { input })
}

export function deleteWorkflowVar(input: DeleteWorkflowVarInput): Promise<null> {
  return call<null>('delete_workflow_var', { input })
}

export function exportWorkflow(id: string): Promise<string> {
  return call<string>('export_workflow', { id })
}

export function importWorkflow(input: ImportWorkflowInput): Promise<SavedWorkflow> {
  return call<SavedWorkflow>('import_workflow', { input })
}

export function previewWorkflowImport(json: string): Promise<WorkflowImportPreview> {
  return call<WorkflowImportPreview>('preview_workflow_import', { json })
}

export function workflowTemplates(): Promise<Array<WorkflowTemplateInfo>> {
  return call<Array<WorkflowTemplateInfo>>('workflow_templates')
}

export function workflowFromTemplate(input: WorkflowFromTemplateInput): Promise<SavedWorkflow> {
  return call<SavedWorkflow>('workflow_from_template', { input })
}

export function saveWorkflowTest(input: SaveWorkflowTestInput): Promise<Workflow> {
  return call<Workflow>('save_workflow_test', { input })
}

export function deleteWorkflowTest(input: DeleteWorkflowTestInput): Promise<Workflow> {
  return call<Workflow>('delete_workflow_test', { input })
}

export function runWorkflowTests(id: string): Promise<Array<WorkflowTestResult>> {
  return call<Array<WorkflowTestResult>>('run_workflow_tests', { id })
}
