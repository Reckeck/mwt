/** Commands: what a period or a spread would do (spec op-period I5-I7); nothing is written. */

import { isFiatCode } from '@/shared/lib/format'
import { txId, txMeta } from '@/shared/lib/transactions'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockTransactions, priceOf } from './demo'
import { checkPeriodMeta, monthShares, periodOfRule, settleSpread } from './period'

const placesOf = (symbol: string) => (isFiatCode(symbol) ? 2 : 8)
const text = (value: number) => String(Number(value.toFixed(2)))

/** The months of the stored operation `id` as an edit to `period` / `spread` would leave them. */
function effectOf(id: string, period?: OpPeriod, spread?: Spread): PeriodEffect {
  const stored = mockTransactions.find((tx) => txId(tx) === id)
  if (!stored) throw { code: 'MISSING', message: 'transaction' }
  const body = 'topup' in stored ? stored.topup : 'payment' in stored ? stored.payment : null
  if (!body) throw { code: 'INVALID_INPUT', message: 'only an income or an expense has a period' }
  const meta = txMeta(stored)
  const nextPeriod = period ?? meta.period
  const nextSpread = settleSpread(nextPeriod, spread ?? meta.spread, body.amount, placesOf(body.asset.symbol), true, {
    amount: body.amount,
    period: meta.period,
    spread: meta.spread,
  })
  checkPeriodMeta(true, nextPeriod, nextSpread, body.amount)
  const value = Number(body.amount) * (Number(body.price.price_usd) || priceOf(body.asset.symbol))
  const moneyMonth = meta.timestamp.slice(0, 7)
  const read = (at?: OpPeriod, shares?: Spread) => {
    const months = new Map<string, number>()
    for (const share of monthShares(at, shares, moneyMonth, value, 'period')) months.set(share.month, share.value)
    return months
  }
  const before = read(meta.period, meta.spread)
  const after = read(nextPeriod, nextSpread)
  const months = [...new Set([...before.keys(), ...after.keys()])].sort()
  return {
    months: months
      .map((month) => ({ month, before: text(before.get(month) ?? 0), after: text(after.get(month) ?? 0) }))
      .filter((row) => row.before !== '0' || row.after !== '0'),
  }
}

export const periodCommands: MockCommands = {
  resolve_period_rule: ({ args }) => resolve(periodOfRule(args?.rule as PeriodRule, String(args?.moneyDate))),
  preview_spread: ({ args }) => {
    const period = args?.period as OpPeriod
    const amount = String(args?.amount)
    const symbol = String((args?.asset as AssetIdent | undefined)?.symbol)
    const given: Spread = { mode: args?.mode as SpreadMode, shares: (args?.shares as MonthShare[] | null) ?? [] }
    const settled = settleSpread(period, given, amount, placesOf(symbol), true)
    checkPeriodMeta(true, period, settled, amount)
    return resolve(settled?.shares ?? [])
  },
  preview_period_change: ({ args }) =>
    resolve(
      effectOf(
        String(args?.id),
        (args?.period as OpPeriod | null) ?? undefined,
        (args?.spread as Spread | null) ?? undefined,
      ),
    ),
}
