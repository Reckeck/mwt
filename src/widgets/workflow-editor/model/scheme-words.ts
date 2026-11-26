import type { WordsNames } from './node-words'

/**
 * `In words` over the canvas (Auto-2 · 13): `when a top-up is added to
 * Tinkoff in the category Salary → ask `Transfer` → transfer 10 %
 * of the amount to Cash`. Events and actions in the order the wires lead, the
 * values bold. The same words a card and `Mine` use for a scheme.
 */
export type WordPart = { text: string; strong?: boolean }

type Translate = (key: string, values?: Record<string, unknown>) => string
export interface WordsNode {
  id: string
  kind: string
  params: Record<string, unknown>
  disabled: boolean
}
export interface WordsEdge {
  source: string
  target: string
  targetHandle?: string | null
}

const VERBS = ['transfer', 'topup', 'payment', 'convert'] as const
const storeOf = (value: unknown): string | null =>
  value && typeof value === 'object' && 'store' in value ? String((value as { store: unknown }).store) : null

/** Triggers first, then whatever their wires reach, breadth first. */
function order(nodes: WordsNode[], edges: WordsEdge[]): WordsNode[] {
  const byId = new Map(nodes.map((node) => [node.id, node]))
  const queue = nodes.filter((node) => node.kind.startsWith('trigger.'))
  const seen = new Set(queue.map((node) => node.id))
  for (let index = 0; index < queue.length; index++) {
    const current = queue[index]
    for (const edge of edges) {
      const next = edge.source === current?.id ? byId.get(edge.target) : undefined
      if (next && !seen.has(next.id)) {
        seen.add(next.id)
        queue.push(next)
      }
    }
  }
  return queue.filter((node) => !node.disabled)
}

export function schemeWords(
  nodes: WordsNode[],
  edges: WordsEdge[],
  title: (kind: string) => string,
  names: WordsNames,
  t: Translate,
): WordPart[] {
  const byId = new Map(nodes.map((node) => [node.id, node]))
  const lower = (kind: string) => title(kind).toLowerCase()
  const steps: WordPart[][] = []
  for (const node of order(nodes, edges)) {
    const { params } = node
    if (node.kind === 'trigger.transaction') {
      const kind = typeof params.tx_kind === 'string' && params.tx_kind !== 'any' ? params.tx_kind : 'any'
      const store = storeOf(params.store)
      const what = `${t(`workflow.words.tx.${kind}`)}${store ? ` ${t('workflow.words.on')} ${names.store(store)}` : ''}`
      const parts: WordPart[] = [{ text: `${t('workflow.words.added')} ` }, { text: what, strong: true }]
      if (typeof params.category === 'string' && params.category) {
        parts.push(
          { text: ` ${t('workflow.words.inCategory')} ` },
          { text: names.category(params.category), strong: true },
        )
      }
      steps.push(parts)
    } else if (node.kind.startsWith('trigger.')) {
      steps.push([{ text: `${t('workflow.words.when')} ` }, { text: lower(node.kind), strong: true }])
    } else if (node.kind === 'action.confirm') {
      const question = String(params.question ?? '').trim()
      steps.push([{ text: t('workflow.words.ask') }, ...(question ? [{ text: ` «${question}»`, strong: true }] : [])])
    } else if (node.kind === 'action.transaction') {
      const kind = String(params.tx_kind ?? 'transfer')
      const verb = (VERBS as readonly string[]).includes(kind) ? t(`workflow.words.verb.${kind}`) : lower(node.kind)
      const wire = edges.find((edge) => edge.target === node.id && edge.targetHandle === 'amount')
      const source = wire ? byId.get(wire.source) : undefined
      const sum =
        source?.kind === 'logic.math' && source.params.op === 'percent'
          ? t('workflow.words.share', { pct: String(source.params.b ?? '') })
          : params.amount && params.amount !== '0'
            ? String(params.amount)
            : ''
      const to = storeOf(params.to)
      const parts: WordPart[] = [{ text: verb }]
      if (sum) parts.push({ text: ' ' }, { text: sum, strong: true })
      if (to) parts.push({ text: ` ${t('workflow.words.on')} ` }, { text: names.store(to), strong: true })
      steps.push(parts)
    } else if (node.kind.startsWith('action.')) {
      steps.push([{ text: lower(node.kind) }])
    }
  }
  if (!steps.length) return []
  const out: WordPart[] = []
  steps.forEach((parts, index) => {
    if (index) out.push({ text: ' → ' })
    out.push(...parts)
  })
  out.push({ text: '.' })
  return out
}
