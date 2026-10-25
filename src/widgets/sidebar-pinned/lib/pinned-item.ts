import type { LocationQuery, RouteLocationRaw } from 'vue-router'

import { entityLocation } from '@/shared/lib/entity-link'
import { formatPct } from '@/shared/lib/format'
import type { SumPart } from '@/shared/lib/money-parts'

/** The short value beside a pin: a sum, or a percent (a goal's progress); nothing when there is none. */
export function pinnedValue(view: PinnedView): SumPart | null {
  if (view.pct !== null) return { text: formatPct(view.pct, { decimals: 0 }) }
  if (view.value === null || !Number(view.value)) return null
  return { amount: view.value, currency: view.currency ?? undefined, decimals: 0 }
}

/** Where a pin leads: the entity's page or card, or the operations with the saved filter. */
export function pinnedLocation(pin: Pinned, query: LocationQuery): RouteLocationRaw {
  if (pin.type === 'entity') return entityLocation(pin.target, query)
  return { name: 'transactions', query: Object.fromEntries(new URLSearchParams(pin.query)) }
}
