import type { RouteLocationRaw } from 'vue-router'

import type { WidgetSize } from '@/shared/ui/widget-frame'

/** Which way of a change is good news: income and capital up, spending down. */
export type MetricGoodWhen = 'up' | 'down'

export interface MetricTileProps {
  label: string
  /** Change against the base, percent; `null` when there is no base yet. */
  deltaPct?: number | string | null
  goodWhen?: MetricGoodWhen
  /** What the change is against: «к прошлой неделе», «к плану». */
  baseLabel?: string
  /** Trend for the sparkline (shown from «M»). */
  series?: Array<number | string>
  /** One-line conclusion (shown from «M»). */
  insight?: string
  /** Where to look closer; the value itself is a link when this is set. */
  action?: { label: string; to: RouteLocationRaw }
  /** Detail level; the cell's own by default. */
  size?: WidgetSize
}
