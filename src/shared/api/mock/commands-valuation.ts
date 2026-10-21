/** Commands: dated valuations of a property and its revalue rule, as `service/valuation.rs` (simplified). */

import { resolve } from './core'
import type { MockCommands } from './core'
import { mockOwnPositions, mockStore } from './tree'

const valuations: Valuation[] = []
const today = () => new Date().toISOString().slice(0, 10)
const fail = (message: string) => {
  throw { code: 'INVALID_INPUT', message }
}

function propertyOf(storeId: string): PropertyInfo {
  const info = mockStore(storeId)?.property
  if (!info) fail('not a property')
  return info as PropertyInfo
}

function propertyValueOf(storeId: string, position: StorePosition): PropertyValue {
  const info = propertyOf(storeId)
  const key = `asset:${position.asset.symbol}`
  const points = valuations
    .filter((row) => row.store_id === storeId && row.asset_key === key)
    .sort((left, right) => left.date.localeCompare(right.date))
  const buyPrice = position.avg_cost ?? info.purchase_price
  const purchase: PropertyPurchase | null = buyPrice
    ? { date: info.purchased_at ?? today(), unit_price: buyPrice, currency: 'usd', from_operation: !!position.avg_cost }
    : null
  const last = points.at(-1)
  const price = last?.unit_price ?? position.price_base
  const change = purchase && price ? Number(price) - Number(purchase.unit_price) : null
  const changePct = change !== null && purchase ? (change / Number(purchase.unit_price)) * 100 : null
  return {
    store_id: storeId,
    asset_key: key,
    symbol: position.asset.symbol,
    quantity: position.quantity,
    currency: last?.currency ?? 'usd',
    unit_price: price,
    purchase,
    base: purchase ? 'purchase' : null,
    base_date: purchase?.date ?? null,
    base_price: purchase?.unit_price ?? null,
    valuations: points,
    rule: info.revalue ?? null,
    change: change === null ? null : change.toFixed(2),
    change_pct: changePct === null ? null : changePct.toFixed(2),
    real_change_pct: changePct === null ? null : (changePct - 8).toFixed(2),
  }
}

export const valuationCommands: MockCommands = {
  property_valuations: ({ args }) => {
    const storeId = String(args?.storeId ?? '')
    propertyOf(storeId)
    const things = mockOwnPositions(storeId).filter((position) => position.asset.chain === null)
    return resolve(things.map((position) => propertyValueOf(storeId, position)))
  },
  property_valuation_add: ({ body }) => {
    const input = body as ValuationInput
    if (Number(input.unit_price) <= 0) fail('price must be positive')
    const kept = valuations.findIndex((row) => row.asset_key === input.asset_key && row.date === input.date)
    if (kept >= 0) valuations.splice(kept, 1)
    const row: Valuation = { id: `val-${Date.now()}`, ...input }
    valuations.push(row)
    return resolve(row)
  },
  property_valuation_delete: ({ args }) => {
    const index = valuations.findIndex((row) => row.id === args?.id)
    if (index < 0) throw { code: 'NOT_FOUND', message: 'valuation' }
    return resolve(valuations.splice(index, 1)[0])
  },
  property_set_revalue: ({ args }) => {
    const rule = (args?.rule as RevalueRule | null) ?? null
    if (rule?.pct_per_year && Number(rule.pct_per_year) < 0 && !rule.floor) fail('a floor is required')
    propertyOf(String(args?.storeId ?? '')).revalue = rule ?? undefined
    return resolve(null)
  },
}
