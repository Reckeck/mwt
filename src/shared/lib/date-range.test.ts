import { describe, expect, it } from 'vitest'
import { formatDateRange, isRangeReversed } from './date-range'

describe('formatDateRange', () => {
  it.each([
    [{ start: '2026-11-02', end: null }, 'ru', '2 ноя'],
    [{ start: '2026-11-02', end: '2026-11-02' }, 'ru', '2 ноя'],
    [{ start: '2026-11-02', end: '2026-11-04' }, 'ru', '2–4 ноя'],
    [{ start: '2026-10-30', end: '2026-11-02' }, 'ru', '30 окт – 2 ноя'],
    [{ start: '2026-12-30', end: '2027-01-02' }, 'ru', '30 дек 2026 – 2 янв 2027'],
    [{ start: '2026-05-02', end: null }, 'ru', '2 мая'],
    [{ start: '2026-10-30', end: '2026-11-02' }, 'en', '30 Oct – 2 Nov'],
    [{ start: null, end: null }, 'ru', ''],
  ])('%o in %s reads %s', (range, locale, expected) => {
    expect(formatDateRange(range, locale)).toBe(expected)
  })
})

describe('isRangeReversed', () => {
  it('flags an end before the start only', () => {
    expect(isRangeReversed({ start: '2026-11-04', end: '2026-11-02' })).toBe(true)
    expect(isRangeReversed({ start: '2026-11-02', end: '2026-11-04' })).toBe(false)
    expect(isRangeReversed({ start: '2026-11-02', end: null })).toBe(false)
  })
})
