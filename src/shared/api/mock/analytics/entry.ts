/** The report by kind, and the calendar of a month. */

import { txLegs, txMeta } from '@/shared/lib/transactions'
import { flows, meta, money, monthsOf } from './flows'
import type { AnalyticsMockContext } from './flows'
import { cashflow, categories, networth, yearReport } from './reports'

export function mockReport(kind: ReportKind, filter: ReportFilter, ctx: AnalyticsMockContext): AnalyticsReport {
  const months = monthsOf(filter, kind)
  switch (kind) {
    case 'cashflow':
      return { cashflow: cashflow(ctx, months, filter) }
    case 'year':
      return { year: yearReport(ctx, months, filter) }
    case 'categories':
      return { categories: categories(ctx, months, filter) }
    case 'networth':
      return { networth: networth(ctx, months, filter) }
    case 'stores':
      return {
        stores: {
          meta: meta(ctx, months, filter),
          rows: ctx.stores.map((store) => ({
            store_id: store.id,
            name: store.name,
            depth: store.parent_id ? 1 : 0,
            kind: store.kind,
            value_start: '0',
            value_end: '0',
            change: '0',
            inflow: '0',
            outflow: '0',
            tx_count: ctx.transactions.filter((tx) => txLegs(tx).some((leg) => leg.store_id === store.id)).length,
          })),
        },
      }
    case 'pnl':
      return {
        pnl: {
          meta: meta(ctx, months, filter),
          rows: months.map((month) => ({ month, realized: '0', fees: '0' })),
          total_realized: '0',
          total_fees: '0',
        },
      }
    case 'portfolio': {
      const worth = networth(ctx, months, filter)
      return {
        portfolio: {
          meta: worth.meta,
          assets: ['USD'],
          points: worth.points.map((point) => ({
            date: `${point.month}-28`,
            total: point.net,
            values: [point.net],
            shares: ['100'],
          })),
        },
      }
    }
    case 'losses':
      return {
        losses: {
          meta: meta(ctx, months, filter),
          rows: months.map((month) => ({ month, fee: '0', slippage: '0', total: '0' })),
          total: { month: '', fee: '0', slippage: '0', total: '0' },
          by_kind: [],
          by_store: [],
          by_contact: [],
          by_flow: [],
          incomplete: 0,
        },
      }
    case 'inflation': {
      const worth = networth(ctx, months, filter)
      return {
        inflation: {
          meta: worth.meta,
          points: worth.points.map((point, index) => {
            const coefficient = 1.03 ** ((months.length - 1 - index) / 12)
            return {
              month: point.month,
              coefficient: money(coefficient),
              nominal_net: point.net,
              real_net: money(Number(point.net) * coefficient),
            }
          }),
          nominal_change_pct: null,
          real_change_pct: null,
        },
      }
    }
  }
}

export function mockCalendar(month: string, ctx: AnalyticsMockContext): CalendarMonth {
  const [year, monthNumber] = month.split('-').map(Number)
  const length = new Date(year, monthNumber, 0).getDate()
  const today = new Date().toISOString().slice(0, 10)
  const all = flows(ctx)
  const allowance = 100
  const days = Array.from({ length }, (_unused, index): CalendarDay => {
    const date = `${month}-${String(index + 1).padStart(2, '0')}`
    const ofDay = all.filter((flow) => flow.date === date)
    const daily = ofDay
      .filter((flow) => !flow.incoming && flow.group !== 'fixed')
      .reduce((sum, flow) => sum + flow.value, 0)
    const spent = ofDay.filter((flow) => !flow.incoming).reduce((sum, flow) => sum + flow.value, 0)
    const status: DayStatus =
      date > today ? 'future' : daily > allowance ? 'over' : daily > allowance * 0.8 ? 'near' : 'under'
    return {
      date,
      income: money(ofDay.filter((flow) => flow.incoming).reduce((sum, flow) => sum + flow.value, 0)),
      spent: money(spent),
      daily: money(daily),
      tx_count: ctx.transactions.filter((tx) => txMeta(tx).timestamp.slice(0, 10) === date).length,
      status,
    }
  })
  return { month, currency: ctx.currency, allowance: String(allowance), days, upcoming: [] }
}
