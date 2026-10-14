import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { localized } from '@/shared/lib/workflow'
import { runsAfter } from '@/shared/lib/workflow-order'

import { type EditorNode, type FlowPortLinks, isNote } from './flow'
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

  const nodeTitle = (id: string) => {
    const nodeDef = def(editor.kindOf(id))
    return editor.nodeById(id)?.data?.label || (nodeDef ? localized(nodeDef.title, locale.value) : id)
  }

  /** The node's flow ports and the wires on each: where the run comes from and goes next. */
  const flowLinks = computed<FlowPortLinks[]>(() => {
    const node = selectedNode.value
    const nodeDef = selectedDef.value
    if (!node || !nodeDef) return []
    const side = (ports: NodeDef['inputs'], isOut: boolean): FlowPortLinks[] =>
      ports
        .filter((port) => port.type === 'flow')
        .map((port) => ({
          port: port.name,
          side: isOut ? 'out' : 'in',
          label: localized(port.label, locale.value),
          links: editor.edges.value
            .filter((edge) =>
              isOut
                ? edge.source === node.id && edge.sourceHandle === port.name
                : edge.target === node.id && edge.targetHandle === port.name,
            )
            .map((edge) => ({ edgeId: edge.id, title: nodeTitle(isOut ? edge.target : edge.source) })),
        }))
    return [...side(nodeDef.inputs, false), ...side(nodeDef.outputs, true)]
  })

  /** The actions the node runs after by data, by name: it has no flow wire in. */
  const dataAfter = computed<string[]>(() => {
    const node = selectedNode.value
    if (!node) return []
    const edges = editor.edges.value.map((edge) => ({
      from: edge.source,
      to: edge.target,
      toPort: edge.targetHandle ?? '',
    }))
    return runsAfter(edges, (id) => def(editor.kindOf(id)), node.id).map(nodeTitle)
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

  return { selectedNode, selectedDef, wiredFrom, flowLinks, dataAfter, setParam, setLabel, unwire }
}
