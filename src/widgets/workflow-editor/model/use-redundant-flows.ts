import { type Ref, computed } from 'vue'
import { useStyleTag } from '@vueuse/core'

import type { EditorGraph } from './use-editor-graph'

/**
 * Flow wires the backend found repeating a data wire (`redundant_flow`): the
 * order stays without them. They are drawn dashed, and one click removes them.
 */
export function useRedundantFlows(editor: EditorGraph, issues: Ref<GraphIssue[]>) {
  const edgeIds = computed(() =>
    issues.value.flatMap((issue) => (issue.code === 'redundant_flow' && issue.edge_id ? [issue.edge_id] : [])),
  )
  const css = computed(() =>
    edgeIds.value
      .map(
        (id) => `.vue-flow__edge[data-id=${JSON.stringify(id)}] .vue-flow__edge-path{stroke-dasharray:6 5;opacity:.6}`,
      )
      .join('\n'),
  )
  useStyleTag(css)

  function removeAll() {
    const drop = new Set(edgeIds.value)
    editor.edges.value = editor.edges.value.filter((edge) => !drop.has(edge.id))
  }

  return { count: computed(() => edgeIds.value.length), removeAll }
}
