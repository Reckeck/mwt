import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { localized } from '@/shared/lib/workflow'

import { type EditorNode, isNote } from './flow'
import type { EditorGraph } from './use-editor-graph'

/** The one node the inspector shows, what feeds each of its inputs, and its edits. */
export function useSelection(editor: EditorGraph) {
  const { locale } = useI18n()
  const { flow, def } = editor

  const selectedNode = computed(() => {
    const selected = flow.getSelectedNodes.value
    return selected.length === 1 && !isNote(selected[0]) ? (selected[0] as EditorNode) : null
  })
  const selectedDef = computed(() => def(selectedNode.value?.data?.kind ?? ''))

  /** Input port → «source node · its output», for the inputs that have a wire. */
  const wiredFrom = computed<Record<string, string>>(() => {
    const node = selectedNode.value
    if (!node) return {}
    const out: Record<string, string> = {}
    for (const edge of editor.edges.value.filter((entry) => entry.target === node.id)) {
      const sourceDef = def(editor.kindOf(edge.source))
      const port = sourceDef?.outputs.find((entry) => entry.name === edge.sourceHandle)
      const title =
        editor.nodeById(edge.source)?.data?.label ||
        (sourceDef ? localized(sourceDef.title, locale.value) : edge.source)
      out[edge.targetHandle ?? ''] = port ? `${title} · ${localized(port.label, locale.value)}` : title
    }
    return out
  })

  function setParam(param: string, value: unknown) {
    const node = selectedNode.value
    if (node) editor.patchNode(node.id, (data) => ({ ...data, params: { ...data.params, [param]: value } }))
  }

  function setLabel(value: string) {
    const node = selectedNode.value
    if (node) editor.patchNode(node.id, (data) => ({ ...data, label: value }))
  }

  function unwire(port: string) {
    const node = selectedNode.value
    if (node) editor.unwire(node.id, port)
  }

  return { selectedNode, selectedDef, wiredFrom, setParam, setLabel, unwire }
}
