/** Analytics mock context, months of a filter and money flows. */

import { txLegs, txMeta } from '@/shared/lib/transactions'

export interface AnalyticsMockContext {
  transactions: Transaction[]
  categories: Category[]
  stores: Store[]
  priceOf: (symbol: string) => number
  currency: string
}

export const money = (value: number) => String(Math.round(value * 100) / 100)
const monthKey = (date: Date) => `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`

export function monthsOf(filter: ReportFilter, kind: ReportKind): string[] {
  const now = new Date()
  if (kind === 'year') {
    const year = filter.year ?? now.getFullYear()
    return Array.from({ length: 12 }, (_unused, index) => `${year}-${String(index + 1).padStart(2, '0')}`)
  }
  const to = filter.to ? new Date(`${filter.to}-01T12:00`) : new Date(now.getFullYear(), now.getMonth(), 1, 12)
  const from = filter.from ? new Date(`${filter.from}-01T12:00`) : new Date(to.getFullYear(), to.getMonth() - 11, 1, 12)
  const out: string[] = []
  for (const cursor = new Date(from); cursor <= to && out.length < 120; cursor.setMonth(cursor.getMonth() + 1)) {
    out.push(monthKey(cursor))
  }
  return out
}

export interface MockFlow {
  month: string
  date: string
  group: CategoryGroup | null
  key: string
  value: number
  incoming: boolean
}

export function flows(ctx: AnalyticsMockContext): MockFlow[] {
  return ctx.transactions.flatMap((tx) => {
    const meta = txMeta(tx)
    const key = meta.categories[0] ?? ''
    const group = ctx.categories.find((entry) => entry.key === key)?.group ?? null
    if (!('topup' in tx) && !('payment' in tx)) return []
    const leg = txLegs(tx).find((entry) => entry.role === 'principal')
    if (!leg) return []
    const value = Math.abs(Number(leg.amount)) * (Number(leg.price_usd) || ctx.priceOf(leg.asset.symbol))
    return [
      {
        month: meta.timestamp.slice(0, 7),
        date: meta.timestamp.slice(0, 10),
        group,
        key,
        value,
        incoming: 'topup' in tx,
      },
    ]
  })
}

export function meta(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): ReportMeta {
  return { currency: ctx.currency, months, real_terms: filter.real_terms, inflation_rate: null }
}
