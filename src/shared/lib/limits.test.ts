import { describe, expect, it } from 'vitest'

import { clampToDateBounds, DATE_MAX, DATE_MIN, isDateWithinBounds, MAX_DATE_YEAR, MIN_DATE_YEAR } from './limits'

describe('isDateWithinBounds', () => {
  it('accepts both edges', () => {
    expect(isDateWithinBounds(`${MIN_DATE_YEAR}-01-01`)).toBe(true)
    expect(isDateWithinBounds(`${MAX_DATE_YEAR}-12-31`)).toBe(true)
    expect(isDateWithinBounds(`${MAX_DATE_YEAR}-12`)).toBe(true)
    expect(isDateWithinBounds(`${MIN_DATE_YEAR}-01-01T00:00`)).toBe(true)
  })
  it('refuses one year past an edge', () => {
    expect(isDateWithinBounds(`${MIN_DATE_YEAR - 1}-12-31`)).toBe(false)
    expect(isDateWithinBounds(`${MAX_DATE_YEAR + 1}-01-01`)).toBe(false)
    expect(isDateWithinBounds(`${MAX_DATE_YEAR + 1}-01`)).toBe(false)
  })
  it('lets blank and unreadable values to the required check', () => {
    expect(isDateWithinBounds('')).toBe(true)
    expect(isDateWithinBounds(null)).toBe(true)
  })
})

describe('clampToDateBounds', () => {
  it('keeps the caller bound when it is tighter, else the global one', () => {
    expect(clampToDateBounds('2020-05-01', 'min')).toBe('2020-05-01')
    expect(clampToDateBounds('1900-01-01', 'min')).toBe(DATE_MIN)
    expect(clampToDateBounds(undefined, 'max')).toBe(DATE_MAX)
    expect(clampToDateBounds('2200-01-01', 'max')).toBe(DATE_MAX)
  })
})
