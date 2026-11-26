import { describe, expect, it } from 'vitest'

import { MAX_DATE_YEAR, MIN_DATE_YEAR } from '@/shared/lib/limits'

import { canStepRange, monthInBounds, yearInBounds } from './period-bounds'

describe('period bounds', () => {
  it('stops the year at both edges', () => {
    expect(yearInBounds(MIN_DATE_YEAR)).toBe(true)
    expect(yearInBounds(MIN_DATE_YEAR - 1)).toBe(false)
    expect(yearInBounds(MAX_DATE_YEAR)).toBe(true)
    expect(yearInBounds(MAX_DATE_YEAR + 1)).toBe(false)
  })

  it('refuses a month outside the years', () => {
    expect(monthInBounds(`${MIN_DATE_YEAR}-01`)).toBe(true)
    expect(monthInBounds(`${MIN_DATE_YEAR - 1}-12`)).toBe(false)
    expect(monthInBounds(`${MAX_DATE_YEAR + 1}-01`)).toBe(false)
  })

  it('stops the arrows where the next period leaves the bounds', () => {
    const first = { from: `${MIN_DATE_YEAR}-01`, to: `${MIN_DATE_YEAR}-03` }
    expect(canStepRange(first, -1)).toBe(false)
    expect(canStepRange(first, 1)).toBe(true)
    const last = { from: `${MAX_DATE_YEAR}-10`, to: `${MAX_DATE_YEAR}-12` }
    expect(canStepRange(last, 1)).toBe(false)
    expect(canStepRange(last, -1)).toBe(true)
  })
})
