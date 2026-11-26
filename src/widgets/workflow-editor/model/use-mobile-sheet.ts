import { onScopeDispose, type Ref, ref, watch } from 'vue'

import type { MobileInspectorTab } from '../workflow-editor.types'
import type { EditorGraph } from './use-editor-graph'

/**
 * When the phone's sheet opens by itself: a tapped node shows its inspector,
 * `Details` on the run card shows the report. Closing it is the user's.
 */
/** Time for a new node to be measured and the canvas's first fit to end. */
const SETTLE_MS = 350

export function useMobileSheet(editor: EditorGraph, report: Ref<unknown>, phone: Ref<boolean>) {
  const open = ref(false)
  const tab = ref<MobileInspectorTab>('inspector')

  function show(next: MobileInspectorTab) {
    tab.value = next
    open.value = true
  }

  /** The sheet covers the lower half: the tapped node moves to the middle of the half above it. */
  function keepInView(id: string) {
    const node = editor.flow.findNode(id)
    const bounds = editor.flow.vueFlowRef.value?.getBoundingClientRect()
    if (!node || !bounds) return
    const zoom = Math.min(editor.flow.getViewport().zoom, 1)
    const centreX = node.computedPosition.x + node.dimensions.width / 2
    const centreY = node.computedPosition.y + node.dimensions.height / 2
    void editor.flow.setCenter(centreX, centreY + bounds.height / 4 / zoom, { zoom, duration: 250 })
  }

  let settle: ReturnType<typeof setTimeout> | undefined
  onScopeDispose(() => clearTimeout(settle))
  watch(
    () => editor.flow.getSelectedNodes.value[0]?.id,
    (id) => {
      if (!phone.value || !id) return
      show('inspector')
      // After the node is measured and the first fit to view is done, or that fit undoes the move.
      clearTimeout(settle)
      settle = setTimeout(() => keepInView(id), SETTLE_MS)
    },
  )
  // A finished check shows as a card under the steps; the sheet's `Run` opens from it.
  watch(report, (next) => {
    if (!next && tab.value === 'run') tab.value = 'inspector'
  })

  return { open, tab, show }
}
