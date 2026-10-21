import type { RouteLocationRaw } from 'vue-router'

/** Last day of a `YYYY-MM` month, `YYYY-MM-DD`. */
export function lastDay(month: string): string {
  const [year, number] = month.split('-').map(Number)
  return `${month}-${String(new Date(year, number, 0).getDate()).padStart(2, '0')}`
}

/** What a clicked figure is made of; every field narrows the operations list. */
export interface DrillContext {
  /** Months `YYYY-MM` (one or a range) or days `YYYY-MM-DD`. */
  from?: string
  to?: string
  /** A category key; `''` = without a category. */
  category?: string
  kinds?: TxKind[]
  storeIds?: string[]
  /** Only operations that paid a fee (losses). */
  withFee?: boolean
  /** Only operations that closed a position (P&L). */
  realized?: boolean
}

const dayFrom = (value: string) => (value.length === 7 ? `${value}-01` : value)
const dayTo = (value: string) => (value.length === 7 ? lastDay(value) : value)

/** The operations behind a figure: one rule for every chart, list and widget. */
export function drillTo(context: DrillContext): RouteLocationRaw {
  const query: Record<string, string> = {}
  const until = context.to ?? context.from
  if (context.from) query.from = dayFrom(context.from)
  if (until) query.to = dayTo(until)
  if (context.category !== undefined) query.cat = context.category
  if (context.storeIds?.length) query.store = context.storeIds.join(',')
  if (context.kinds?.length) query.kind = context.kinds.join(',')
  if (context.withFee) query.fee = '1'
  if (context.realized) query.realized = '1'
  return { name: 'transactions', query }
}

/** The local day `YYYY-MM-DD` of a moment; `exclusive` takes the day before an end bound. */
export function localDay(iso: string, exclusive = false): string {
  const moment = new Date(new Date(iso).getTime() - (exclusive ? 1 : 0))
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${moment.getFullYear()}-${pad(moment.getMonth() + 1)}-${pad(moment.getDate())}`
}
