/** «Куда ушли»: the core's `where_went` over the mock history. */

import { txLegs, txMeta } from '@/shared/lib/transactions'
import { mockContacts } from '../ledger/seed'
import { flows, meta, money } from './flows'
import type { AnalyticsMockContext } from './flows'

const ITEMS = 6

function row(kind: WhereKind, items: WhereItem[], count = items.length): WhereRow {
  const value = items.reduce((sum, item) => sum + Number(item.value ?? 0), 0)
  const sorted = [...items].sort((left, right) => Number(right.value ?? 0) - Number(left.value ?? 0))
  return {
    kind,
    value: money(value),
    count,
    items: sorted.slice(0, ITEMS),
    partial: items.some((item) => item.value === null),
  }
}

const item = (part: Partial<WhereItem>): WhereItem => ({
  label: '',
  to: null,
  qty: '0',
  unit: '',
  got_qty: null,
  got_unit: null,
  value: null,
  comment: '',
  at: '',
  ...part,
})

export function whereReport(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): WhereReport {
  const inPeriod = (stamp: string) => months.includes(stamp.slice(0, 7))
  const name = (account: AccountRef) =>
    'store' in account
      ? (ctx.stores.find((store) => store.id === account.store)?.name ?? '')
      : 'contact' in account
        ? (mockContacts.find((contact) => contact.id === account.contact.contact_id)?.name ?? '')
        : ''
  const priced = (qty: string, unit: string) => money(Math.abs(Number(qty)) * ctx.priceOf(unit))
  const period = flows(ctx).filter((flow) => months.includes(flow.month))
  const income = period
    .filter((flow) => flow.incoming && flow.group === 'income')
    .reduce((sum, flow) => sum + flow.value, 0)
  const spent = new Map<string, number>()
  const outgoing = period.filter((flow) => !flow.incoming && flow.group !== 'service')
  for (const flow of outgoing) spent.set(flow.key, (spent.get(flow.key) ?? 0) + flow.value)
  const lent: WhereItem[] = []
  const moved: WhereItem[] = []
  const invested: WhereItem[] = []
  for (const tx of ctx.transactions) {
    const { timestamp, comment } = txMeta(tx)
    if (!inPeriod(timestamp)) continue
    if ('transfer' in tx) {
      const body = tx.transfer
      const base = {
        qty: body.amount,
        unit: body.asset.symbol,
        value: priced(body.amount, body.asset.symbol),
        comment,
        at: timestamp,
      }
      if (body.debt === 'lend') lent.push(item({ ...base, label: name(body.to), to: name(body.from) }))
      else if (!body.debt && 'store' in body.from && 'store' in body.to)
        moved.push(item({ ...base, label: name(body.from), to: name(body.to) }))
    } else if ('convert' in tx && 'store' in tx.convert.from.account && 'store' in tx.convert.to.account) {
      const { from, to } = tx.convert
      moved.push(
        item({
          label: name(from.account),
          to: name(to.account),
          qty: from.amount,
          unit: from.asset.symbol,
          got_qty: to.amount,
          got_unit: to.asset.symbol,
          value: priced(from.amount, from.asset.symbol),
          comment,
          at: timestamp,
        }),
      )
    } else if ('trade_spot' in tx) {
      const body = tx.trade_spot
      const store = ctx.stores.find((entry) => entry.id === body.store_id)?.name ?? ''
      invested.push(
        item({
          label: store,
          qty: body.sold.qty,
          unit: body.sold.asset.symbol,
          got_qty: body.bought.qty,
          got_unit: body.bought.asset.symbol,
          value: priced(body.sold.qty, body.sold.asset.symbol),
          comment,
          at: timestamp,
        }),
      )
    }
  }
  // Fees of the period (network, exchange): the core's «Потери».
  const fees = ctx.transactions
    .filter((tx) => inPeriod(txMeta(tx).timestamp))
    .flatMap((tx) => txLegs(tx).filter((leg) => leg.role === 'fee'))
  const lost = row(
    'losses',
    fees.map((leg) =>
      item({
        label: leg.asset.symbol,
        qty: leg.amount,
        unit: leg.asset.symbol,
        value: priced(leg.amount, leg.asset.symbol),
      }),
    ),
  )
  const spending = row(
    'spending',
    [...spent].map(([label, value]) => item({ label, value: money(value) })),
    outgoing.length,
  )
  const rows = [spending, row('lent', lent), row('moved', moved), row('invested', invested), lost].filter(
    (entry) => entry.count > 0,
  )
  return { meta: meta(ctx, months, filter), income: money(income), rows, left: money(income - Number(spending.value)) }
}
