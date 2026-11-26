import { nextTick } from 'vue'

import { shortId } from '@/shared/lib/workflow'

import { toGraph } from './flow'
import type { EditorGraph } from './use-editor-graph'

/** Copy, paste and duplicate of the selected nodes with the wires between them. */
export function useGraphClipboard(editor: EditorGraph) {
  const { flow, nodes, edges } = editor
  let clipboard: WorkflowGraph | null = null

  function copySelected() {
    const ids = new Set(flow.getSelectedNodes.value.map((node) => node.id))
    if (!ids.size) return
    clipboard = toGraph(
      nodes.value.filter((node) => ids.has(node.id)),
      edges.value.filter((edge) => ids.has(edge.source) && ids.has(edge.target)),
    )
  }

  function paste(offset = 40) {
    if (!clipboard) return
    const fresh = new Map(clipboard.nodes.map((node) => [node.id, shortId('n')]))
    const copies = clipboard.nodes.map((node) => ({
      ...editor.newNode(node.kind, { x: node.position.x + offset, y: node.position.y + offset }, node.params),
      id: fresh.get(node.id) ?? shortId('n'),
      data: { kind: node.kind, params: { ...node.params }, label: node.label, disabled: node.disabled },
    }))
    nodes.value = [...nodes.value, ...copies]
    for (const edge of clipboard.edges) {
      editor.connect(fresh.get(edge.from) ?? '', edge.from_port, fresh.get(edge.to) ?? '', edge.to_port)
    }
    flow.removeSelectedElements()
    void nextTick(() =>
      flow.addSelectedNodes(nodes.value.filter((node) => copies.some((copy) => copy.id === node.id)) as never),
    )
  }

  function duplicate() {
    copySelected()
    paste()
  }

  return { copySelected, paste, duplicate }
}
