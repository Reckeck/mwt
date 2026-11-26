import { describe, expect, it } from 'vitest'

import { overrunOf } from './overrun'

function status(category: string, remaining: string, currency = 'rub', active = true): BudgetStatus {
  return {
    budget: { id: 'b', active, target: { category }, period: 'month', limit: '1000', currency, warn: true },
    spent: '0',
    remaining,
    used: '0',
    period_start: '',
    period_end: '',
    not_valued: false,
  }
}

describe('overrunOf', () => {
  it('says how far over the limit the expense goes', () => {
    expect(overrunOf([status('food', '300')], 'food', '500', 'RUB')).toEqual({ over: '200', currency: 'rub' })
  })
  it('is quiet while the expense fits', () => {
    expect(overrunOf([status('food', '300')], 'food', '300', 'rub')).toBeNull()
  })
  it('ignores another category, another currency and an inactive limit', () => {
    expect(overrunOf([status('food', '1')], 'taxi', '500', 'rub')).toBeNull()
    expect(overrunOf([status('food', '1', 'usd')], 'food', '500', 'rub')).toBeNull()
    expect(overrunOf([status('food', '1', 'rub', false)], 'food', '500', 'rub')).toBeNull()
  })
})
