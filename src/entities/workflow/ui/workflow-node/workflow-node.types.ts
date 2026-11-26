/** What the canvas hands a node card. */
export interface WorkflowNodeData {
  def: NodeDef | undefined
  kind: string
  label: string
  disabled: boolean
  /** Errors and warnings pinned to this node. */
  issues: GraphIssue[]
  /** Its value in words, e.g. `amount × 10 %`, `Transfer → Cash`. */
  summary: string
  /** What keeps the scheme off, over the card in red: `fill in `To``; empty when nothing does. */
  problem: string
  /** Wired ports, «in:a» / «out:result»: only these show until the node is selected or hovered. */
  linked: string[]
  /** Last emulation: this node's step. */
  step: TraceStep | null
  /** Last emulation: this node's place in the run, 1…; `null` when it was skipped or not reached. */
  order: number | null
}
