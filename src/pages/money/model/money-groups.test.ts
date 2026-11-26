import { describe, expect, it } from 'vitest'

import { budgetTabs } from './money-groups'

describe('budgetTabs', () => {
  it('keeps the named children that have a group, in order', () => {
    const children = [
      { meta: {} },
      { name: 'a', meta: {} },
      { name: 'b', meta: { group: 'plan' } },
      { name: 'c', meta: { group: 'month' } },
    ]
    expect(budgetTabs(children)).toEqual([
      { name: 'b', labelKey: 'money.tab.plan' },
      { name: 'c', labelKey: 'money.tab.month' },
    ])
  })
})
