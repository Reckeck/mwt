/** What the canvas hands a node card. */
export interface WorkflowNodeData {
  def: NodeDef | undefined
  kind: string
  label: string
  disabled: boolean
  /** Errors and warnings pinned to this node. */
  issues: GraphIssue[]
  /** A line about its fields, e.g. «каждый месяц, 10-е, 09:00». */
  summary: string
  /** Last emulation: this node's step. */
  step: TraceStep | null
}
