import { describe, expect, it } from 'vitest'

import { groupByDay } from './group-by-day'

const asset = { symbol: 'USD', kind: 'fiat' }
const meta = (id: string, timestamp: string) => ({
  id,
  timestamp,
  comment: '',
  categories: [],
  flow_id: null,
  contact_id: null,
  origin: 'manual',
})
const payment = (id: string, timestamp: string, amount: string, priceUsd: string | null) =>
  ({
    payment: {
      meta: meta(id, timestamp),
      store_id: 'cash',
      to: null,
      asset,
      amount,
      fee: null,
      price: priceUsd ? { price_usd: priceUsd } : null,
    },
  }) as unknown as Transaction
const transfer = (id: string, timestamp: string, from: string, to: string) =>
  ({
    transfer: {
      meta: meta(id, timestamp),
      from: { store: from },
      to: { store: to },
      asset,
      amount: '50',
      fee: null,
      price: { price_usd: '1' },
    },
  }) as unknown as Transaction

const rootOf = (id: string) => id.split('/')[0]
const noon = (day: string) => new Date(`${day}T12:00:00`).toISOString()

describe('groupByDay', () => {
  it('groups consecutive transactions by local day and keeps the order they come in', () => {
    const days = groupByDay(
      [
        payment('a', noon('2026-11-26'), '10', '1'),
        payment('b', noon('2026-11-26'), '5', '1'),
        payment('c', noon('2026-11-25'), '7', '1'),
        payment('d', noon('2026-11-26'), '1', '1'),
      ],
      1,
      rootOf,
    )
    expect(days.map((day) => day.date)).toEqual(['2026-11-26', '2026-11-25', '2026-11-26'])
    expect(days[0].txs.map((tx) => Object.values(tx)[0].meta.id)).toEqual(['a', 'b'])
  })

  it('totals what came in minus what went out, at the price times the rate', () => {
    const [day] = groupByDay([payment('a', noon('2026-11-26'), '10', '2')], 3, rootOf)
    expect(day.totalBase).toBe(-60)
    expect(day.partial).toBe(false)
  })

  it('leaves a move between own wallets of one root out of the total', () => {
    const [day] = groupByDay(
      [transfer('t', noon('2026-11-26'), 'bank/card', 'bank/savings'), payment('a', noon('2026-11-26'), '10', '1')],
      1,
      rootOf,
    )
    expect(day.txs).toHaveLength(2)
    expect(day.totalBase).toBe(-10)
  })

  it('marks the day partial when a leg has no price and counts that transaction not at all', () => {
    const [day] = groupByDay(
      [payment('a', noon('2026-11-26'), '10', null), payment('b', noon('2026-11-26'), '4', '1')],
      1,
      rootOf,
    )
    expect(day.partial).toBe(true)
    expect(day.totalBase).toBe(-4)
  })
})
