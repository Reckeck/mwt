import { describe, expect, it } from 'vitest'

import { quickDebtEffect } from './debt-effect'

describe('quickDebtEffect', () => {
  it('lends on a transfer marked as a loan', () => {
    expect(quickDebtEffect('transfer', true)).toBe('lend')
  })
  it('is none otherwise', () => {
    expect(quickDebtEffect('transfer', false)).toBeNull()
    expect(quickDebtEffect('payment', true)).toBeNull()
  })
})
