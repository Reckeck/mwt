import { type EditorEdge, type EditorNode, isNote } from './flow'

/** The phone's list (Auto-2 · 13): the nodes on the flow wires are steps, the rest is data feeding them. */
export interface SchemeSteps {
  /** In run order: from the triggers along the flow wires, then the unwired by place. */
  steps: string[]
  data: string[]
}

export function schemeSteps(
  nodes: EditorNode[],
  edges: EditorEdge[],
  def: (kind: string) => NodeDef | undefined,
): SchemeSteps {
  const real = nodes.filter((node) => !isNote(node))
  const isStep = (node: EditorNode) => {
    const nodeDef = def(node.data?.kind ?? '')
    return Boolean(nodeDef && [...nodeDef.inputs, ...nodeDef.outputs].some((port) => port.type === 'flow'))
  }
  const byPlace = (left: EditorNode, right: EditorNode) =>
    left.position.y - right.position.y || left.position.x - right.position.x
  const candidates = real.filter(isStep).sort(byPlace)
  const stepIds = new Set(candidates.map((node) => node.id))
  const next = new Map<string, string[]>()
  const fed = new Set<string>()
  for (const edge of edges) {
    if (edge.data?.type !== 'flow' || !stepIds.has(edge.source) || !stepIds.has(edge.target)) continue
    next.set(edge.source, [...(next.get(edge.source) ?? []), edge.target])
    fed.add(edge.target)
  }
  const order: string[] = []
  const queue = candidates.filter((node) => !fed.has(node.id)).map((node) => node.id)
  while (queue.length) {
    const id = queue.shift() as string
    if (order.includes(id)) continue
    order.push(id)
    queue.push(...(next.get(id) ?? []))
  }
  for (const node of candidates) if (!order.includes(node.id)) order.push(node.id)
  return {
    steps: order,
    data: real
      .filter((node) => !stepIds.has(node.id))
      .sort(byPlace)
      .map((node) => node.id),
  }
}
