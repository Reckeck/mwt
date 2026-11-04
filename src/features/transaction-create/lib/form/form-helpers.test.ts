import { describe, expect, it } from 'vitest'
import { stampToPrice } from './form-helpers'

const stamp = (over: Partial<PriceStamp>): PriceStamp => ({
  price_usd: null,
  price_base: null,
  price_base_currency: null,
  fx_override: null,
  ...over,
})

describe('stampToPrice', () => {
  it('takes the currency of the label for a base price written by another device', () => {
    expect(stampToPrice(stamp({ price_usd: '100', price_base: '8000', price_base_currency: 'rub' }))).toEqual({
      price: '8000',
      priceCurrency: 'rub',
    })
  })

  it('leaves the currency empty for an old base price without a label', () => {
    expect(stampToPrice(stamp({ price_usd: '100', price_base: '8000' }))).toEqual({ price: '8000', priceCurrency: '' })
  })

  it('falls back to USD without a base price', () => {
    expect(stampToPrice(stamp({ price_usd: '100' }))).toEqual({ price: '100', priceCurrency: 'usd' })
  })
})
