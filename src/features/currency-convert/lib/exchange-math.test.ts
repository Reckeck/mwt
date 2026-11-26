import { describe, expect, it } from 'vitest'

import { exchangeLoss, exchangeRate } from './exchange-math'

describe('exchangeRate', () => {
  it('is got per one given', () => {
    expect(exchangeRate('100', '92')).toBe('0.92')
  })
  it('is null without both amounts', () => {
    expect(exchangeRate('', '92')).toBeNull()
    expect(exchangeRate('100', '0')).toBeNull()
  })
})

describe('exchangeLoss', () => {
  it('is the market value minus what was got', () => {
    expect(exchangeLoss('95', '92')).toBe('3')
  })
  it('is negative when the deal beats the market', () => {
    expect(exchangeLoss('90', '92')).toBe('-2')
  })
  it('is null without a market value', () => {
    expect(exchangeLoss(null, '92')).toBeNull()
  })
})
