import { DASHBOARD_COLS, DASHBOARD_WIDGETS } from '../dashboard-registry.constants'
import type { DashboardWidgetDef } from '../dashboard-registry.types'

export function widgetDef(id: string): DashboardWidgetDef | undefined {
  return DASHBOARD_WIDGETS.find((def) => def.id === id)
}

/**
 * Place widgets left to right, top to bottom in a 12-column grid. The grid
 * compacts upwards afterwards, so this only needs to avoid overlaps.
 */
export function packWidgets(ids: string[]): DashboardWidgetCfg[] {
  const out: DashboardWidgetCfg[] = []
  let column = 0
  let row = 0
  let rowHeight = 0
  for (const id of ids) {
    const def = widgetDef(id)
    if (!def) continue
    if (column + def.w > DASHBOARD_COLS) {
      column = 0
      row += rowHeight
      rowHeight = 0
    }
    out.push({ id, col: column, row, width: def.w, height: def.h, pinned: false, store_id: null, period: null })
    column += def.w
    rowHeight = Math.max(rowHeight, def.h)
  }
  return out
}
