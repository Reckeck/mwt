export interface WorkflowEditorProps {
  /** The workflow to edit; `null` for a new one. */
  workflowId?: string | null
  /** A template from `WORKFLOW_TEMPLATES` to start a new one from. */
  template?: string | null
}
