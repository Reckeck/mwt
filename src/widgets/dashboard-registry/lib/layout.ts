import type { Layout } from 'grid-layout-plus'

import type { DashboardWidgetExtras } from '../dashboard-registry.types'
import { packWidgets, widgetDef } from './registry'

/**
 * Widgets merged into one (plan H2). A saved layout is read through this map
 * for good: an old vault may be opened years later.
 */
export const MERGED_WIDGETS: Readonly<Record<string, string>> = {
  total: 'netWorth',
  utilitiesDue: 'utilities',
  utilityStats: 'utilities',
  fixedCosts: 'utilities',
  spending7d: 'spending',
  miniCalendar: 'spending',
  byChain: 'allocation',
  byStore: 'allocation',
}

/** Whether a saved layout still names a merged widget, so it is worth writing back once. */
export const hasMergedIds = (saved: DashboardWidgetCfg[]) => saved.some((cfg) => cfg.id in MERGED_WIDGETS)

/**
 * Old ids become the widget they were merged into; of several that land on one id
 * the first in reading order stays, grown to the new widget's minimum.
 */
function renamed(saved: DashboardWidgetCfg[]): DashboardWidgetCfg[] {
  if (!hasMergedIds(saved)) return saved
  const seen = new Set<string>()
  const order = [...saved].sort((left, right) => left.row - right.row || left.col - right.col)
  return order.flatMap((cfg) => {
    const id = MERGED_WIDGETS[cfg.id] ?? cfg.id
    if (seen.has(id)) return []
    seen.add(id)
    const def = widgetDef(id)
    return [{ ...cfg, id, width: Math.max(cfg.width, def?.minW ?? 1), height: Math.max(cfg.height, def?.minH ?? 1) }]
  })
}

/**
 * A widget narrower than its minimum can only come from a narrow-window
 * layout saved over the real one (every widget squeezed into one column).
 * Such a layout is packed afresh in its reading order; pins and store
 * bindings are kept.
 */
export function repaired(raw: DashboardWidgetCfg[]): DashboardWidgetCfg[] {
  const saved = renamed(raw).filter((cfg) => widgetDef(cfg.id))
  const damaged = saved.some((cfg) => cfg.width < (widgetDef(cfg.id)?.minW ?? 1))
  if (!damaged) return saved
  const order = [...saved].sort((left, right) => left.row - right.row || left.col - right.col)
  const byId = new Map(saved.map((cfg) => [cfg.id, cfg]))
  return packWidgets(order.map((cfg) => cfg.id)).map((cfg) => ({
    ...cfg,
    pinned: byId.get(cfg.id)?.pinned ?? false,
    store_id: byId.get(cfg.id)?.store_id ?? null,
    period: byId.get(cfg.id)?.period ?? null,
  }))
}

/** Grid items of the full-width layout. */
export function toLayout(cfgs: DashboardWidgetCfg[]): Layout {
  return cfgs.map((cfg) => {
    const def = widgetDef(cfg.id)
    return {
      i: cfg.id,
      x: cfg.col,
      y: cfg.row,
      w: cfg.width,
      h: cfg.height,
      minW: def?.minW,
      minH: def?.minH,
      static: cfg.pinned,
      isResizable: !cfg.pinned,
    }
  })
}

/** The grid's items back as stored widgets, with their pins, wallet bindings and own periods. */
export function toCfgs(
  items: Layout,
  pinned: Set<string>,
  extras: Map<string, DashboardWidgetExtras>,
): DashboardWidgetCfg[] {
  return items.map((item) => ({
    id: String(item.i),
    col: item.x,
    row: item.y,
    width: item.w,
    height: item.h,
    pinned: pinned.has(String(item.i)),
    store_id: extras.get(String(item.i))?.store_id ?? null,
    period: extras.get(String(item.i))?.period ?? null,
  }))
}

/** Order-independent fingerprint: a save that changes nothing is skipped, and so is the re-layout it would cause. */
export function layoutSignature(cfgs: DashboardWidgetCfg[]): string {
  return JSON.stringify(
    cfgs
      .map((cfg) => [
        cfg.id,
        cfg.col,
        cfg.row,
        cfg.width,
        cfg.height,
        cfg.pinned,
        cfg.store_id ?? null,
        cfg.period ?? null,
      ])
      .sort((left, right) => String(left[0]).localeCompare(String(right[0]))),
  )
}
