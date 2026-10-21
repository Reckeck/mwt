import type { RouteLocationRaw } from 'vue-router'

import { drillTo } from '@/shared/lib/drill'

export interface DrillDown {
  /** One month, `YYYY-MM`; or a range of them. */
  from?: string
  to?: string
  /** A category key; `''` = without a category. */
  category?: string
  /** Kinds of transactions the figure is made of. */
  kinds?: TxKind[]
  /** Only transactions that paid a fee (losses). */
  withFee?: boolean
  /** Only transactions that closed a position (P&L). */
  realized?: boolean
}

/**
 * The transactions behind a figure: the list filtered by its months, its
 * category, kinds and the report's wallets.
 */
export function transactionsOf(target: DrillDown, filter: ReportFilter): RouteLocationRaw {
  return drillTo({ ...target, storeIds: filter.scope.store_ids })
}
