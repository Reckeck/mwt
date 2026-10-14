import { type Ref, onBeforeUnmount, ref, watch } from 'vue'
import { type DrawingKind, DrawingManager } from 'lightweight-charts-drawing'

import { tfLabel } from '../lib/timeframe'
import type { ChartHandles } from './use-chart-core'

/**
 * Lines, rectangles and notes on the chart, per symbol: loaded from the saved
 * export, reported back on every change (`onChange`), with undo and clear.
 */
export function useChartDrawings(options: {
  symbol: () => string
  saved: () => Record<string, string>
  tf: Ref<number>
  onChange: (symbol: string, json: string) => void
}) {
  let manager: DrawingManager | null = null
  /** Guard so importJSON/clear do not feed back into persistence. */
  let applying = false

  const activeTool = ref<DrawingKind | null>(null)
  const magnet = ref(true)
  const count = ref(0)
  const undoStack = ref<string[]>([])

  function persist() {
    if (applying || !manager) return
    options.onChange(options.symbol(), manager.exportJSON())
  }

  /** The current symbol's saved drawings; a broken export just opens empty. */
  function load() {
    if (!manager) return
    applying = true
    try {
      manager.clear()
      manager.importJSON(options.saved()[options.symbol().toUpperCase()] ?? '[]')
    } catch {
      // A broken export must not take the chart down.
    } finally {
      applying = false
    }
    undoStack.value = []
    count.value = manager.drawings().length
  }

  function init(handles: ChartHandles) {
    if (!handles.chart || !handles.candles) return
    const current = new DrawingManager(handles.chart, handles.candles, { magnet: magnet.value ? 'weak' : 'off' })
    manager = current
    current.setInterval(tfLabel(options.tf.value))
    const recount = () => {
      count.value = current.drawings().length
    }
    current.on('add', (drawing) => {
      undoStack.value.push(drawing.id)
      recount()
      persist()
    })
    current.on('remove', (drawing) => {
      undoStack.value = undoStack.value.filter((id) => id !== drawing.id)
      recount()
      persist()
    })
    current.on('change', () => {
      recount()
      persist()
    })
    current.on('tool', () => {
      activeTool.value = current.tool() ?? null
    })
    load()
  }

  /** A single-choice group emits '' when the active item is pressed again: that means "no tool". */
  function pickTool(value: unknown) {
    const next = (typeof value === 'string' && value ? value : null) as DrawingKind | null
    manager?.setTool(next)
    activeTool.value = next
  }

  function undo() {
    const id = undoStack.value.pop()
    if (id) manager?.remove(id)
  }

  function clear() {
    if (!manager) return
    applying = true
    try {
      manager.clear()
    } finally {
      applying = false
    }
    undoStack.value = []
    count.value = 0
    persist()
  }

  watch(magnet, (on) => manager?.setMagnet(on ? 'weak' : 'off'))
  watch(options.tf, (minutes) => manager?.setInterval(tfLabel(minutes)))
  onBeforeUnmount(() => {
    manager?.destroy()
    manager = null
  })

  return { activeTool, magnet, count, undoStack, init, load, pickTool, undo, clear }
}
