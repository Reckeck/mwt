/**
 * Data order, as the backend runs it (`Graph::data_order`): a node with a flow
 * input but no flow wire into it runs after the actions its data inputs read,
 * directly or through pure data nodes. Triggers do not count: they always ran.
 * Shared by the editor and the browser mock.
 */
export interface OrderEdge {
  from: string
  to: string
  toPort: string
}

export type DefOf = (nodeId: string) => NodeDef | undefined

const hasFlow = (def: NodeDef) => [...def.inputs, ...def.outputs].some((port) => port.type === 'flow')
const isFlowInput = (def: NodeDef | undefined, port: string) =>
  def?.inputs.some((input) => input.name === port && input.type === 'flow') ?? false

/** Whether a flow wire comes into the node. */
export function hasFlowIn(edges: OrderEdge[], defOf: DefOf, nodeId: string): boolean {
  const def = defOf(nodeId)
  return edges.some((edge) => edge.to === nodeId && isFlowInput(def, edge.toPort))
}

/** The actions whose outputs reach the node's data inputs. */
export function actionSources(edges: OrderEdge[], defOf: DefOf, nodeId: string): string[] {
  const found: string[] = []
  const seen = new Set<string>()
  const walk = (id: string) => {
    const def = defOf(id)
    for (const edge of edges.filter((entry) => entry.to === id)) {
      if (!def || isFlowInput(def, edge.toPort) || seen.has(edge.from)) continue
      seen.add(edge.from)
      const from = defOf(edge.from)
      if (!from) continue
      if (!hasFlow(from)) walk(edge.from)
      else if (from.group !== 'trigger') found.push(edge.from)
    }
  }
  walk(nodeId)
  return found
}

/** The actions a node runs after by data: empty when a flow wire leads into it. */
export function runsAfter(edges: OrderEdge[], defOf: DefOf, nodeId: string): string[] {
  const def = defOf(nodeId)
  if (!def || def.group === 'trigger' || !def.inputs.some((port) => port.type === 'flow')) return []
  return hasFlowIn(edges, defOf, nodeId) ? [] : actionSources(edges, defOf, nodeId)
}
