import { describe, expect, it } from 'vitest'

import { alertInputOf, alertInputOfFields, fieldsOfAlert } from './alert-fields'

const alert = {
  id: 'a1',
  asset: { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin', chain: null, contract: null, extra: 1 },
  condition: 'below',
  price: '50000',
  currency: 'usd',
  active: true,
  triggered_at: null,
  store_id: 's1',
  repeats: true,
} as unknown as PriceAlert

describe('alert fields', () => {
  it('round-trips every field into the stored input', () => {
    const input = alertInputOfFields(fieldsOfAlert(alert), { active: alert.active, storeId: alert.store_id })
    expect(input).toEqual(alertInputOf(alert))
  })

  it('the base keeps only the identity of the asset', () => {
    expect(alertInputOf(alert).asset).toEqual({
      symbol: 'BTC',
      name: 'Bitcoin',
      coingecko_id: 'bitcoin',
      chain: null,
      contract: null,
    })
  })
})
