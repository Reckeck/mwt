import type { Edge as FlowEdge, Node as FlowNode } from '@vue-flow/core'

import { GRAPH_VERSION, portStroke, portType } from '@/shared/lib/workflow'

/** What a canvas node carries: the stored node without its position. */
export interface EditorNodeData {
  kind: string
  params: Record<string, unknown>
  label: string
  disabled: boolean
}

/** A note on the canvas: text only, no effect on a run. */
export interface EditorNoteData {
  text: string
  color: string
}

export type EditorNode = FlowNode<EditorNodeData>
export type EditorNote = FlowNode<EditorNoteData>

/** Notes and nodes share the canvas; notes carry this id prefix. */
export const NOTE_PREFIX = 'note-'

export function isNote(node: { type?: string }): boolean {
  return node.type === 'note'
}

export function toFlowNote(comment: WorkflowComment): EditorNode {
  return {
    id: comment.id.startsWith(NOTE_PREFIX) ? comment.id : `${NOTE_PREFIX}${comment.id}`,
    type: 'note',
    position: { x: comment.position.x, y: comment.position.y },
    data: { text: comment.text, color: comment.color } as unknown as EditorNodeData,
  }
}
export type EditorEdge = FlowEdge<{ type: PortType }>

/** One flow port of the selected node and the wires on it, for the inspector. */
export interface FlowPortLinks {
  port: string
  side: 'in' | 'out'
  label: string
  links: Array<{ edgeId: string; title: string }>
}

/** How a wire looks: a thick orange line for the flow, a dashed one in the port's colour for data. */
export function edgeStyle(type: PortType): Pick<EditorEdge, 'style' | 'animated' | 'class'> {
  return {
    style:
      type === 'flow'
        ? { stroke: 'var(--wire-flow)', strokeWidth: 2.5 }
        : { stroke: portStroke(type), strokeWidth: 1.75, strokeDasharray: '5 4' },
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
    nodes: [...graph.comments.map(toFlowNote), ...graph.nodes.map(toFlowNode)],
    edges: graph.edges.map((edge) => toFlowEdge(edge, defs, (id) => kinds.get(id) ?? '')),
  }
}

/** The stored graph of what is on the canvas. */
export function toGraph(all: EditorNode[], edges: EditorEdge[]): WorkflowGraph {
  const nodes = all.filter((node) => !isNote(node))
  const comments: WorkflowComment[] = all.filter(isNote).map((note) => {
    const data = note.data as unknown as EditorNoteData | undefined
    return {
      id: note.id,
      text: data?.text ?? '',
      position: { x: Math.round(note.position.x), y: Math.round(note.position.y) },
      width: 0,
      height: 0,
      color: data?.color ?? '',
    }
  })
  return {
    version: GRAPH_VERSION,
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
