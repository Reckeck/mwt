import { type Ref, computed, nextTick, ref } from 'vue'
import { type Connection, useVueFlow } from '@vue-flow/core'

import type { useNodeCatalog } from '@/entities/workflow'
import { portAccepts, portType, shortId } from '@/shared/lib/workflow'

import {
  type EditorEdge,
  type EditorNode,
  type EditorNoteData,
  NOTE_PREFIX,
  isNote,
  toFlow,
  toFlowEdge,
  toGraph,
} from './flow'

export type EditorGraph = ReturnType<typeof useEditorGraph>
type NodeData = NonNullable<EditorNode['data']>

/**
 * The canvas content and every edit on it: nodes and wires, joining ports by
 * type, adding, selecting, removing, notes. The other editor parts work on
 * this one object.
 */
export function useEditorGraph(catalog: Pick<ReturnType<typeof useNodeCatalog>, 'defs' | 'def'>) {
  const { defs, def } = catalog
  const flowId = `workflow-${Math.random().toString(36).slice(2)}`
  const flow = useVueFlow(flowId)

  // Typed as plain refs: unwrapping Vue Flow's node type is too deep for TypeScript.
  const nodes = ref([]) as Ref<EditorNode[]>
  const edges = ref([]) as Ref<EditorEdge[]>
  const graph = computed(() => toGraph(nodes.value, edges.value))
  const graphJson = computed(() => JSON.stringify(graph.value))

  function loadGraph(source: WorkflowGraph) {
    const next = toFlow(source, defs.value)
    nodes.value = next.nodes
    edges.value = next.edges
  }

  /* --- wiring --- */
  const nodeById = (id: string) => nodes.value.find((node) => node.id === id)
  const kindOf = (id: string) => nodeById(id)?.data?.kind ?? ''
  const outType = (nodeId: string, port: string | null | undefined) => portType(def(kindOf(nodeId)), port ?? '', 'out')
  const inType = (nodeId: string, port: string | null | undefined) => portType(def(kindOf(nodeId)), port ?? '', 'in')

  function isValidConnection(connection: Connection): boolean {
    if (connection.source === connection.target) return false
    const from = outType(connection.source, connection.sourceHandle)
    const to = inType(connection.target, connection.targetHandle)
    return Boolean(from && to && portAccepts(to, from))
  }

  /** Join two ports; a data input keeps one wire, so a new one replaces the old. */
  function connect(source: string, sourcePort: string, target: string, targetPort: string) {
    if (inType(target, targetPort) !== 'flow') {
      edges.value = edges.value.filter((edge) => !(edge.target === target && edge.targetHandle === targetPort))
    }
    const id = `${source}.${sourcePort}-${target}.${targetPort}`
    if (edges.value.some((edge) => edge.id === id)) return
    edges.value = [
      ...edges.value,
      toFlowEdge({ id, from: source, from_port: sourcePort, to: target, to_port: targetPort }, defs.value, kindOf),
    ]
  }

  function removeEdge(edgeId: string) {
    edges.value = edges.value.filter((edge) => edge.id !== edgeId)
  }

  function unwire(nodeId: string, port: string) {
    edges.value = edges.value.filter((edge) => !(edge.target === nodeId && edge.targetHandle === port))
  }

  /* --- adding --- */
  function newNode(kind: string, position: { x: number; y: number }, params: Record<string, unknown> = {}): EditorNode {
    return { id: shortId('n'), type: 'wf', position, data: { kind, params: { ...params }, label: '', disabled: false } }
  }

  /** Screen point → canvas point. */
  function toCanvas(clientX: number, clientY: number) {
    const bounds = flow.vueFlowRef.value?.getBoundingClientRect()
    return flow.project({ x: clientX - (bounds?.left ?? 0), y: clientY - (bounds?.top ?? 0) })
  }

  function centerPoint() {
    const bounds = flow.vueFlowRef.value?.getBoundingClientRect()
    return bounds ? { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 3 } : { x: 400, y: 200 }
  }

  function add(kind: string, at = centerPoint(), params: Record<string, unknown> = {}): EditorNode {
    const node = newNode(kind, toCanvas(at.x, at.y), params)
    nodes.value = [...nodes.value, node]
    return node
  }

  /* --- selection --- */
  function select(nodeId: string) {
    flow.removeSelectedElements()
    void nextTick(() => {
      const target = nodeById(nodeId)
      if (target) flow.addSelectedNodes([target as never])
    })
  }

  function focusNode(nodeId: string) {
    const node = nodeById(nodeId)
    if (!node) return
    select(nodeId)
    void flow.setCenter(node.position.x + 120, node.position.y + 60, { zoom: 1, duration: 300 })
  }

  function patchNode(nodeId: string, patch: (data: NodeData) => NodeData) {
    nodes.value = nodes.value.map((node) =>
      node.id === nodeId && node.data ? { ...node, data: patch(node.data) } : node,
    )
  }

  function removeSelected() {
    const ids = new Set(flow.getSelectedNodes.value.map((node) => node.id))
    const edgeIds = new Set(flow.getSelectedEdges.value.map((edge) => edge.id))
    nodes.value = nodes.value.filter((node) => !ids.has(node.id))
    edges.value = edges.value.filter((edge) => !edgeIds.has(edge.id) && !ids.has(edge.source) && !ids.has(edge.target))
  }

  function toggleSelected() {
    for (const node of flow.getSelectedNodes.value.filter((entry) => !isNote(entry))) {
      patchNode(node.id, (data) => ({ ...data, disabled: !data.disabled }))
    }
  }

  /* --- notes --- */
  function addNote() {
    const at = centerPoint()
    const note: EditorNode = {
      id: shortId(NOTE_PREFIX),
      type: 'note',
      position: toCanvas(at.x, at.y),
      data: { text: '', color: '' } as unknown as NodeData,
    }
    nodes.value = [...nodes.value, note]
  }

  function noteText(id: string, text: string) {
    nodes.value = nodes.value.map((node) =>
      node.id === id
        ? { ...node, data: { ...(node.data as unknown as EditorNoteData), text } as unknown as NodeData }
        : node,
    )
  }

  return {
    removeEdge,
    flowId,
    flow,
    def,
    defs,
    nodes,
    edges,
    graph,
    graphJson,
    loadGraph,
    nodeById,
    kindOf,
    outType,
    inType,
    isValidConnection,
    connect,
    unwire,
    newNode,
    centerPoint,
    add,
    select,
    focusNode,
    patchNode,
    removeSelected,
    toggleSelected,
    addNote,
    noteText,
  }
}
