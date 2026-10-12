import type { Edge as FlowEdge, Node as FlowNode } from '@vue-flow/core'

import { portStroke, portType } from '@/shared/lib/workflow'

/** What a canvas node carries: the stored node without its position. */
export interface EditorNodeData {
  kind: string
  params: Record<string, unknown>
  label: string
  disabled: boolean
}

export type EditorNode = FlowNode<EditorNodeData>
export type EditorEdge = FlowEdge<{ type: PortType }>

/** How a wire looks: a thick neutral line for the flow, a coloured one for data. */
export function edgeStyle(type: PortType): Pick<EditorEdge, 'style' | 'animated' | 'class'> {
  return {
    style: { stroke: portStroke(type), strokeWidth: type === 'flow' ? 2.5 : 1.5 },
    animated: false,
    class: type === 'flow' ? 'wf-edge-flow' : 'wf-edge-data',
  }
}

export function toFlowNode(node: WorkflowNode): EditorNode {
  return {
    id: node.id,
    type: 'wf',
    position: { x: node.position.x, y: node.position.y },
    data: { kind: node.kind, params: { ...node.params }, label: node.label, disabled: node.disabled },
  }
}

export function toFlowEdge(edge: WorkflowEdge, defs: Map<string, NodeDef>, kindOf: (id: string) => string): EditorEdge {
  const type = portType(defs.get(kindOf(edge.from)), edge.from_port, 'out') ?? 'any'
  return {
    id: edge.id,
    source: edge.from,
    sourceHandle: edge.from_port,
    target: edge.to,
    targetHandle: edge.to_port,
    data: { type },
    ...edgeStyle(type),
  }
}

export function toFlow(graph: WorkflowGraph, defs: Map<string, NodeDef>): { nodes: EditorNode[]; edges: EditorEdge[] } {
  const kinds = new Map(graph.nodes.map((node) => [node.id, node.kind]))
  return {
    nodes: graph.nodes.map(toFlowNode),
    edges: graph.edges.map((edge) => toFlowEdge(edge, defs, (id) => kinds.get(id) ?? '')),
  }
}

/** The stored graph of what is on the canvas. */
export function toGraph(nodes: EditorNode[], edges: EditorEdge[], comments: WorkflowComment[] = []): WorkflowGraph {
  return {
    version: 1,
    nodes: nodes.map((node) => ({
      id: node.id,
      kind: node.data?.kind ?? '',
      params: { ...(node.data?.params ?? {}) },
      position: { x: Math.round(node.position.x), y: Math.round(node.position.y) },
      disabled: node.data?.disabled ?? false,
      label: node.data?.label ?? '',
    })),
    edges: edges.map((edge) => ({
      id: edge.id,
      from: edge.source,
      from_port: edge.sourceHandle ?? '',
      to: edge.target,
      to_port: edge.targetHandle ?? '',
    })),
    comments,
  }
}
