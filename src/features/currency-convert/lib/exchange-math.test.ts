import { describe, expect, it } from 'vitest'

import { exchangeQuote } from './exchange-math'

describe('exchangeQuote', () => {
  it('tells the rate, the market rate and the loss in the given currency', () => {
    expect(exchangeQuote('45000', '480', '488.6')).toEqual({
      rate: '93.75',
      market: '92.0998772',
      loss: '792.06',
      percent: '1.8',
    })
  })
  it('a deal better than the market loses a negative amount', () => {
    const quote = exchangeQuote('100', '92', '90')
    expect(quote?.loss?.startsWith('-')).toBe(true)
    expect(quote?.percent?.startsWith('-')).toBe(true)
  })
  it('without the market rate only the deal rate is known', () => {
    expect(exchangeQuote('100', '92', null)).toEqual({ rate: '1.08695652', market: null, loss: null, percent: null })
  })
  it('needs both amounts', () => {
    expect(exchangeQuote('', '92', '90')).toBeNull()
    expect(exchangeQuote('100', '0', '90')).toBeNull()
  })
})
