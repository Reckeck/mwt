import type { RouteLocationNormalizedLoaded } from 'vue-router'

import { formatMonthKey } from '@/shared/lib/format'

type Translate = (key: string) => string

/** `YYYY-MM` or `YYYY-MM-DD` of the query, when the whole period is one month. */
function monthOf(route: RouteLocationNormalizedLoaded): string | null {
  const from = typeof route.query.from === 'string' ? route.query.from.slice(0, 7) : ''
  const to = typeof route.query.to === 'string' ? route.query.to.slice(0, 7) : from
  return from && from === to ? from : null
}

/**
 * What a recent place reads in Ctrl K (Paths 1 · 7): `Money › Spending · October`
 * — the section, its tab and the month it was filtered to.
 */
export function placeTitle(route: RouteLocationNormalizedLoaded, title: string, t: Translate): string {
  const group = typeof route.meta.group === 'string' ? route.meta.group : ''
  const name = String(route.name ?? '')
  const base = name.startsWith('money') && group ? `${title} › ${t(`money.tab.${group}`)}` : title
  const month = monthOf(route)
  return month ? `${base} · ${formatMonthKey(month, { month: 'long' })}` : base
}
