import { describe, expect, it } from 'vitest'

import { budgetInputOf, budgetInputOfFields, fieldsOfBudget } from './budget-fields'

const budget = (target: BudgetTarget): Budget => ({
  id: 'b1',
  active: false,
  target,
  period: 'week',
  limit: '120.50',
  currency: 'eur',
  warn: false,
})

describe('budget fields', () => {
  it.each<BudgetTarget>(['total', { category: 'groceries' }, { store: { store_ids: ['s1'], include_children: true } }])(
    'round-trips every field for %j',
    (target) => {
      const stored = budget(target)
      expect(budgetInputOfFields(fieldsOfBudget(stored))).toEqual(budgetInputOf(stored))
    },
  )

  it('the base carries no id and keeps the stored values', () => {
    expect(budgetInputOf(budget('total'))).toEqual({
      active: false,
      target: 'total',
      period: 'week',
      limit: '120.50',
      currency: 'eur',
      warn: false,
    })
  })
})
