import { describe, expect, it } from 'vitest'

import { matchingBudgets, overrunOf } from './overrun'

function status(target: BudgetTarget, remaining = '300', currency = 'rub', active = true): BudgetStatus {
  return {
    budget: { id: 'b', active, target, period: 'month', limit: '1000', currency, warn: true },
    spent: '0',
    remaining,
    used: '0',
    period_start: '',
    period_end: '',
    not_valued: false,
  } as BudgetStatus
}
const store = { store: { store_ids: ['s1'] } } as unknown as BudgetTarget

describe('matchingBudgets', () => {
  it('takes the total, the category and the store budget of the wallet', () => {
    const list = [status('total'), status({ category: 'food' }), status(store), status({ category: 'taxi' })]
    expect(matchingBudgets(list, 'food', 's1')).toHaveLength(3)
    expect(matchingBudgets(list, 'food', 's2')).toHaveLength(2)
  })
  it('ignores an inactive budget', () => {
    expect(matchingBudgets([status('total', '1', 'rub', false)], 'food', 's1')).toEqual([])
  })
})

describe('overrunOf', () => {
  it('says how far over the limit the converted expense goes', () => {
    expect(overrunOf(status('total'), '500')).toEqual({ id: 'b', over: '200', currency: 'rub' })
  })
  it('is quiet while the expense fits, without a rate, or without an amount', () => {
    expect(overrunOf(status('total'), '300')).toBeNull()
    expect(overrunOf(status('total'), null)).toBeNull()
    expect(overrunOf(status('total'), '0')).toBeNull()
  })
})
