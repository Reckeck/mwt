import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryLabel } from '@/shared/lib/categories'
import { localized } from '@/shared/lib/workflow'
import { flattenStoreTree } from '@/shared/lib/store-tree'

import { isNote } from './flow'
import { type WiredInputs, type WordsNames, nodeWords } from './node-words'
import { schemeWords } from './scheme-words'
import type { EditorGraph } from './use-editor-graph'

/**
 * The scheme read aloud: «Словами» over the canvas, each node's value in
 * words, and which of its ports are wired — a card shows only those.
 */
export function useSchemeWords(editor: EditorGraph) {
  const { t, locale } = useI18n()
  const tree = useStoreTree()
  const categories = useCategories()

  const names = computed<WordsNames>(() => {
    const stores = new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node.store.name]))
    const cats = new Map((categories.data.value ?? []).map((entry) => [entry.key, categoryLabel(entry, t)]))
    return {
      ru: locale.value.startsWith('ru'),
      store: (id) => stores.get(id) ?? id,
      category: (key) => cats.get(key) ?? key,
    }
  })

  const title = (kind: string) => {
    const def = editor.def(kind)
    return def ? localized(def.title, locale.value) : kind
  }
  const graphNodes = computed(() =>
    editor.nodes.value
      .filter((node) => !isNote(node))
      .map((node) => ({
        id: node.id,
        kind: node.data?.kind ?? '',
        params: node.data?.params ?? {},
        disabled: node.data?.disabled ?? false,
      })),
  )
  const words = computed(() => schemeWords(graphNodes.value, editor.edges.value, title, names.value, t))

  /** Input port → the label of the output wired into it. */
  function wired(id: string): WiredInputs {
    const out: WiredInputs = {}
    for (const edge of editor.edges.value) {
      if (edge.target !== id || !edge.targetHandle) continue
      const source = editor.nodes.value.find((node) => node.id === edge.source)
      const port = editor.def(source?.data?.kind ?? '')?.outputs.find((entry) => entry.name === edge.sourceHandle)
      if (port) out[edge.targetHandle] = localized(port.label, locale.value).toLowerCase()
    }
    return out
  }

  /** «in:a», «out:result»: the ports with a wire. */
  const linked = computed(() => {
    const out = new Map<string, Set<string>>()
    const add = (id: string, key: string) => out.set(id, (out.get(id) ?? new Set()).add(key))
    for (const edge of editor.edges.value) {
      add(edge.source, `out:${edge.sourceHandle ?? ''}`)
      add(edge.target, `in:${edge.targetHandle ?? ''}`)
    }
    return out
  })

  const summaryOf = (id: string, kind: string, params: Record<string, unknown>) =>
    nodeWords(editor.def(kind), params, wired(id), names.value, locale.value)

  return { words, wired, linked, summaryOf }
}
