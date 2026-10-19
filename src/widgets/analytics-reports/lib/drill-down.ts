import type { RouteLocationRaw } from 'vue-router'

/** Last day of a `YYYY-MM` month, `YYYY-MM-DD`. */
function lastDay(month: string): string {
  const [year, number] = month.split('-').map(Number)
  return `${month}-${String(new Date(year, number, 0).getDate()).padStart(2, '0')}`
}

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
  const query: Record<string, string> = {}
  if (target.from) query.from = `${target.from}-01`
  if (target.to ?? target.from) query.to = lastDay((target.to ?? target.from) as string)
  if (target.category !== undefined) query.cat = target.category
  if (filter.scope.store_ids.length) query.store = filter.scope.store_ids.join(',')
  if (target.kinds?.length) query.kind = target.kinds.join(',')
  if (target.withFee) query.fee = '1'
  if (target.realized) query.realized = '1'
  return { name: 'transactions', query }
}
