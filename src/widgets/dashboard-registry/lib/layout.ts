import type { Layout } from 'grid-layout-plus'

import { packWidgets, widgetDef } from './registry'

/**
 * A widget narrower than its minimum can only come from a narrow-window
 * layout saved over the real one (every widget squeezed into one column).
 * Such a layout is packed afresh in its reading order; pins and store
 * bindings are kept.
 */
export function repaired(saved: DashboardWidgetCfg[]): DashboardWidgetCfg[] {
  const damaged = saved.some((cfg) => cfg.width < (widgetDef(cfg.id)?.minW ?? 1))
  if (!damaged) return saved
  const order = [...saved].sort((left, right) => left.row - right.row || left.col - right.col)
  const byId = new Map(saved.map((cfg) => [cfg.id, cfg]))
  return packWidgets(order.map((cfg) => cfg.id)).map((cfg) => ({
    ...cfg,
    pinned: byId.get(cfg.id)?.pinned ?? false,
    store_id: byId.get(cfg.id)?.store_id ?? null,
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

/** The grid's items back as stored widgets, with their pins and wallet bindings. */
export function toCfgs(items: Layout, pinned: Set<string>, bound: Map<string, string | null>): DashboardWidgetCfg[] {
  return items.map((item) => ({
    id: String(item.i),
    col: item.x,
    row: item.y,
    width: item.w,
    height: item.h,
    pinned: pinned.has(String(item.i)),
    store_id: bound.get(String(item.i)) ?? null,
  }))
}

/** Order-independent fingerprint: a save that changes nothing is skipped, and so is the re-layout it would cause. */
export function layoutSignature(cfgs: DashboardWidgetCfg[]): string {
  return JSON.stringify(
    cfgs
      .map((cfg) => [cfg.id, cfg.col, cfg.row, cfg.width, cfg.height, cfg.pinned, cfg.store_id ?? null])
      .sort((left, right) => String(left[0]).localeCompare(String(right[0]))),
  )
}
