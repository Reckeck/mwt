import { describe, expect, it } from 'vitest'

import { monthBalanceFilter } from './month-filter'

describe('monthBalanceFilter', () => {
  it('spans the twelve months ending with the month, over every wallet', () => {
    const filter = monthBalanceFilter('2026-11')
    expect(filter.from).toBe('2025-12')
    expect(filter.to).toBe('2026-11')
    expect(filter.scope.store_ids).toEqual([])
  })
})
