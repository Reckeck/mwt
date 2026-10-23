/** «Вложения» like `service/investments.rs`: coins joined by ticker, a futures sub-account per row. */

import { txLegs } from '@/shared/lib/transactions'

import { mockStores, mockTransactions, priceOf } from './demo'

const STABLE = new Set(['USDT', 'USDC', 'FDUSD', 'DAI', 'USDE', 'BUSD', 'TUSD', 'PYUSD'])
const round = (value: number) => String(Math.round(value * 100) / 100)

/** A trade's bought leg without a price costs what was sold for it. */
function pricedLegs(tx: Transaction) {
  const legs = txLegs(tx)
  const sold = legs.find((leg) => leg.role === 'principal' && Number(leg.amount) < 0)
  return legs.map((leg) => {
    if (leg.price_usd !== null || !sold || leg.role !== 'principal' || Number(leg.amount) <= 0) return leg
    const paid = -Number(sold.amount) * Number(sold.price_usd ?? priceOf(sold.asset.symbol))
    return { ...leg, price_usd: String(paid / Number(leg.amount)) }
  })
}

export function mockInvestments(storeIds: string[]): Investments {
  const rows = new Map<string, { row: InvestmentRow; qty: number; cost: number; value: number }>()
  const stores = mockStores.filter(
    (store) =>
      !store.archived &&
      (store.kind === 'crypto' || store.kind === 'exchange') &&
      (!storeIds.length ||
        storeIds.includes(store.id) ||
        (store.parent_id !== null && storeIds.includes(store.parent_id))),
  )
  for (const store of stores) {
    const held = new Map<string, { asset: AssetRef; qty: number; bought: number; paid: number }>()
    for (const leg of mockTransactions.flatMap(pricedLegs)) {
      if (leg.store_id !== store.id || leg.asset.contract?.startsWith('perp:')) continue
      const key = leg.asset.symbol.toUpperCase()
      const entry = held.get(key) ?? { asset: leg.asset, qty: 0, bought: 0, paid: 0 }
      const qty = Number(leg.amount)
      entry.qty += qty
      if (qty > 0) {
        entry.bought += qty
        entry.paid += qty * Number(leg.price_usd ?? priceOf(key))
      }
      held.set(key, entry)
    }
    if (store.sub_type === 'futures') {
      const margin = [...held.values()].find((entry) => entry.qty > 0)
      if (!margin) continue
      const value = margin.qty * priceOf(margin.asset.symbol)
      rows.set(store.id, {
        row: {
          label: store.name,
          asset: null,
          store_id: store.id,
          quantity: '',
          unit: margin.asset.symbol,
          invested: '',
          value: '',
          profit: '0',
          cost_known: true,
        },
        qty: margin.qty,
        cost: value,
        value,
      })
      continue
    }
    for (const [key, entry] of held) {
      if (entry.qty <= 0 || entry.asset.chain === 'fiat' || STABLE.has(key)) continue
      const cost = entry.bought ? (entry.paid / entry.bought) * entry.qty : 0
      const total = rows.get(key) ?? {
        row: {
          label: key,
          asset: entry.asset,
          store_id: null,
          quantity: '',
          unit: key,
          invested: '',
          value: '',
          profit: '',
          cost_known: true,
        },
        qty: 0,
        cost: 0,
        value: 0,
      }
      total.qty += entry.qty
      total.cost += cost
      total.value += entry.qty * priceOf(key)
      rows.set(key, total)
    }
  }
  const list = [...rows.values()]
    .map(({ row, qty, cost, value }) => ({
      ...row,
      quantity: String(qty),
      invested: round(cost),
      value: round(value),
      profit: row.store_id ? '0' : round(value - cost),
    }))
    .sort((left, right) => Number(right.invested) - Number(left.invested))
  const sum = (pick: (row: InvestmentRow) => string) => round(list.reduce((total, row) => total + Number(pick(row)), 0))
  return {
    base_currency: 'usd',
    invested: sum((row) => row.invested),
    value: sum((row) => row.value),
    profit: sum((row) => row.profit),
    rows: list,
  }
}
