export interface WorkflowEditorProps {
  /** The workflow to edit; `null` for a new one. */
  workflowId?: string | null
  /** A template from `WORKFLOW_TEMPLATES` to start a new one from. */
  template?: string | null
  /** A lesson from `WORKFLOW_LESSONS`: the canvas starts from its task. */
  lesson?: string | null
}

/** A tab of the phone editor's sheet. */
export type MobileInspectorTab = 'inspector' | 'run' | 'lesson'
