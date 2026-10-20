/**
 * Workflow helpers shared by the editor and the browser mock: which ports a
 * wire may join (the same rule as `PortType::accepts` in Rust), texts in the
 * user's language, and the graph's JSON.
 */

/** Whether an output of type `from` may feed an input of type `to`. */
export function portAccepts(to: PortType, from: PortType): boolean {
  if (to === 'flow' || from === 'flow') return to === from
  return (
    to === from ||
    to === 'any' ||
    from === 'any' ||
    (to === 'number' && from === 'money') ||
    (to === 'currency' && from === 'asset') ||
    to === 'text'
  )
}

/** A catalog text in the current language. */
export function localized(text: LocalizedText, locale: string): string {
  return locale.startsWith('ru') ? text.ru : text.en
}

export const NODE_GROUPS: NodeGroup[] = ['trigger', 'data', 'date', 'text', 'logic', 'variable', 'action', 'list']

/** Tailwind classes of a port's dot, by type. */
export const PORT_COLOR: Record<PortType, string> = {
  flow: 'bg-port-flow',
  any: 'bg-port-any',
  number: 'bg-port-number',
  money: 'bg-port-money',
  asset: 'bg-port-asset',
  currency: 'bg-port-currency',
  account: 'bg-port-account',
  category: 'bg-port-category',
  bool: 'bg-port-bool',
  text: 'bg-port-text',
  date: 'bg-port-date',
  duration: 'bg-port-duration',
  transaction: 'bg-port-transaction',
  list: 'bg-port-list',
  record: 'bg-port-record',
}

/** CSS colour of a port type, for wires drawn as SVG. */
export const portStroke = (type: PortType) => `var(--port-${type})`

/** Tailwind classes of a node's header stripe, by group. */
export const GROUP_COLOR: Record<NodeGroup, string> = {
  trigger: 'bg-node-trigger',
  data: 'bg-node-data',
  logic: 'bg-node-logic',
  variable: 'bg-node-variable',
  action: 'bg-node-action',
  list: 'bg-node-list',
  date: 'bg-node-date',
  text: 'bg-node-text',
}

/** The graph format the app writes; mirrors `GRAPH_VERSION` in `graph.rs`. */
export const GRAPH_VERSION = 3

/** An empty graph of the current format. */
export function emptyGraph(): WorkflowGraph {
  return { version: GRAPH_VERSION, nodes: [], edges: [], comments: [] }
}

export function parseGraph(json: string): WorkflowGraph {
  if (!json.trim()) return emptyGraph()
  try {
    const parsed = JSON.parse(json) as Partial<WorkflowGraph>
    return {
      version: parsed.version ?? GRAPH_VERSION,
      nodes: parsed.nodes ?? [],
      edges: parsed.edges ?? [],
      comments: parsed.comments ?? [],
    }
  } catch {
    return emptyGraph()
  }
}

/** A short random id for a node or a wire. */
export function shortId(prefix: string): string {
  return `${prefix}${Math.random().toString(36).slice(2, 8)}`
}

/** The catalog by kind. */
export function catalogMap(catalog: NodeDef[]): Map<string, NodeDef> {
  return new Map(catalog.map((def) => [def.kind, def]))
}

/** The type of a node's port, or `null` when there is no such port. */
export function portType(def: NodeDef | undefined, port: string, side: 'in' | 'out'): PortType | null {
  const list = side === 'in' ? def?.inputs : def?.outputs
  return list?.find((entry) => entry.name === port)?.type ?? null
}

/** The value of a run in words: the trace step's outcome as a key of `workflow.outcome.*`. */
export function outcomeKey(
  outcome: RunOutcome,
): 'done' | 'stopped' | 'failed' | 'waiting' | 'asking' | 'partial' | 'dropped' {
  if (outcome === 'done') return 'done'
  if ('stopped' in outcome) return 'stopped'
  if ('failed' in outcome || 'rejected' in outcome) return 'failed'
  if ('asking' in outcome) return 'asking'
  if ('partial' in outcome) return 'partial'
  if ('dropped' in outcome) return 'dropped'
  return 'waiting'
}

/** The question a run stopped at, when it waits for «Да»/«Нет». */
export function outcomeQuestion(outcome: RunOutcome): { nodeId: string; question: string } | null {
  if (outcome === 'done' || !('asking' in outcome)) return null
  return { nodeId: outcome.asking.node_id, question: outcome.asking.question }
}

/**
 * The reason or the moment a run outcome carries. A dropped run gives its
 * reason code (`off`, `node_removed`, `rate_limit`): `workflow.dropped.<code>`.
 */
export function outcomeDetail(outcome: RunOutcome): string {
  if (outcome === 'done') return ''
  if ('stopped' in outcome) return outcome.stopped.reason
  if ('failed' in outcome) return outcome.failed.message
  if ('asking' in outcome) return outcome.asking.question
  if ('partial' in outcome) return outcome.partial.failed.join('; ')
  if ('dropped' in outcome) return outcome.dropped.reason
  if ('rejected' in outcome) return rejectText(outcome.rejected.reason)
  return outcome.waiting.until
}

/** A refusal for want of money, in figures: «8000 / 25000 RUB». */
function rejectText(reason: RejectReason): string {
  if ('insufficient_funds' in reason) {
    const { available, required, asset } = reason.insufficient_funds
    return `${available} / ${required} ${asset.symbol}`
  }
  if ('credit_limit_exceeded' in reason) {
    const { required, limit } = reason.credit_limit_exceeded
    return `${required} > ${limit}`
  }
  return reason.invalid.message
}

/** Query keys of everything workflows show. */
export const WORKFLOW_KEYS = {
  catalog: ['workflow-catalog'],
  list: ['workflows'],
  one: (id: string) => ['workflow', id],
  runs: ['workflow-runs'],
  queue: ['workflow-queue'],
  waits: ['workflow-waits'],
  vars: ['workflow-vars'],
} as const
