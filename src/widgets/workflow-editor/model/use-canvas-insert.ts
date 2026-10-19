import { ref } from 'vue'

import { portAccepts } from '@/shared/lib/workflow'
import type { QuickAddChoice, QuickAddContext } from '@/features/workflow-quick-add'

import type { EditorGraph } from './use-editor-graph'

interface QuickAddState {
  left: number
  top: number
  at: { x: number; y: number }
  context: QuickAddContext
}

/**
 * Every way a node lands on the canvas: the palette (click or drag), the node
 * search (Ctrl+Space / Shift+A) on a wire, after a node or free, and a wire
 * dropped on empty canvas.
 */
export function useCanvasInsert(editor: EditorGraph) {
  const { flow, def, connect, add, select, outType, inType } = editor
  const quick = ref<QuickAddState | null>(null)
  const pointer = ref(editor.centerPoint())

  function openQuickAdd(clientX: number, clientY: number, context: QuickAddContext) {
    const left = Math.min(clientX, window.innerWidth - 340)
    const top = Math.min(clientY, window.innerHeight - 420)
    quick.value = { left, top, at: { x: clientX, y: clientY }, context }
  }

  /** Ctrl+Space where the cursor is: on a selected wire, after a selected node, or free. */
  function quickAddHere() {
    const at = pointer.value
    const edge = flow.getSelectedEdges.value[0]
    if (edge) {
      const fromType = outType(edge.source, edge.sourceHandle)
      const toType = inType(edge.target, edge.targetHandle)
      if (fromType && toType) {
        openQuickAdd(at.x, at.y, { mode: 'edge', edgeId: edge.id, fromType, toType })
        return
      }
    }
    const selected = flow.getSelectedNodes.value[0]
    const selectedDef = def(selected?.data?.kind ?? '')
    const output = selectedDef?.outputs.find((port) => port.name === 'out') ?? selectedDef?.outputs[0]
    if (selected && output) {
      openQuickAdd(at.x, at.y, { mode: 'port', nodeId: selected.id, port: output.name, type: output.type, side: 'out' })
      return
    }
    openQuickAdd(at.x, at.y, { mode: 'free' })
  }

  /** From the inspector: the search next to the button, joined to this flow port. */
  function quickAddFrom(nodeId: string, port: string, side: 'out' | 'in', event: MouseEvent) {
    const type = side === 'out' ? outType(nodeId, port) : inType(nodeId, port)
    if (type) openQuickAdd(event.clientX, event.clientY, { mode: 'port', nodeId, port, type, side })
  }

  /**
   * After an action's «Дальше»: when the new node takes one of the action's
   * values, a data wire joins them and is enough to order them (data order).
   */
  function joinByData(sourceId: string, port: string, targetId: string): boolean {
    const source = def(editor.kindOf(sourceId))
    const target = def(editor.kindOf(targetId))
    if (port !== 'out' || !source || !target || source.group === 'trigger') return false
    for (const output of source.outputs.filter((entry) => entry.type !== 'flow')) {
      const input = target.inputs.find((entry) => entry.type !== 'flow' && portAccepts(entry.type, output.type))
      if (input) {
        connect(sourceId, output.name, targetId, input.name)
        return true
      }
    }
    return false
  }

  function onQuickPick(choice: QuickAddChoice) {
    const state = quick.value
    quick.value = null
    if (!state) return
    const node = add(choice.kind, state.at, choice.params)
    const context = state.context
    if (context.mode === 'port' && choice.port) {
      if (context.side === 'in') connect(node.id, choice.port, context.nodeId, context.port)
      // A data wire, when there is one to draw, orders them by itself.
      else if (!joinByData(context.nodeId, context.port, node.id))
        connect(context.nodeId, context.port, node.id, choice.port)
    } else if (context.mode === 'edge' && choice.port) {
      const old = editor.edges.value.find((edge) => edge.id === context.edgeId)
      const source = old && editor.nodeById(old.source)
      const target = old && editor.nodeById(old.target)
      // Inserted into a wire: it sits halfway along it.
      if (source && target) {
        node.position = {
          x: Math.round((source.position.x + target.position.x) / 2),
          y: Math.round((source.position.y + target.position.y) / 2) + 40,
        }
      }
      if (old && choice.outPort) {
        editor.edges.value = editor.edges.value.filter((edge) => edge.id !== old.id)
        connect(old.source, old.sourceHandle ?? '', node.id, choice.port)
        connect(node.id, choice.outPort, old.target, old.targetHandle ?? '')
      }
    }
    select(node.id)
  }

  function onPointerMove(event: PointerEvent) {
    pointer.value = { x: event.clientX, y: event.clientY }
  }

  /* --- wires drawn by hand --- */
  let connected = false
  let dragFrom: { nodeId: string; port: string; side: 'out' | 'in' } | null = null
  flow.onConnectStart((params) => {
    connected = false
    dragFrom =
      params.nodeId && params.handleId
        ? { nodeId: params.nodeId, port: params.handleId, side: params.handleType === 'target' ? 'in' : 'out' }
        : null
  })
  flow.onConnect((connection) => {
    connected = true
    connect(connection.source, connection.sourceHandle ?? '', connection.target, connection.targetHandle ?? '')
  })
  flow.onConnectEnd((event) => {
    // Dropped on empty canvas: offer the nodes that fit this port.
    if (connected || !dragFrom || !event) return
    const point = 'clientX' in event ? event : event.changedTouches[0]
    const type =
      dragFrom.side === 'out' ? outType(dragFrom.nodeId, dragFrom.port) : inType(dragFrom.nodeId, dragFrom.port)
    if (!type) return
    openQuickAdd(point.clientX, point.clientY, { mode: 'port', ...dragFrom, type })
  })

  /** The «+» on a phone's canvas: no cursor, so the new node lands in the middle of the view. */
  function quickAddCentre() {
    pointer.value = editor.centerPoint()
    quickAddHere()
  }

  return { quick, quickAddHere, quickAddCentre, quickAddFrom, onQuickPick, onPointerMove }
}
