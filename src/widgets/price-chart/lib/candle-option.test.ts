import { describe, expect, it } from 'vitest'

import { candleExtremes, candleLabel } from './candle-option'

describe('candleLabel', () => {
  it('shows the hour for a short step and the day for a daily one', () => {
    expect(candleLabel('2026-11-26T14:00:00.000Z', 60)).toBe('11-26 14:00')
    expect(candleLabel('2026-11-26T14:00:00.000Z', 1440)).toBe('2026-11-26')
  })
})

describe('candleExtremes', () => {
  it('picks the highest and the lowest close', () => {
    expect(candleExtremes(['a', 'b', 'c'], [2, 9, 1])).toEqual([
      { at: 'b', value: 9 },
      { at: 'c', value: 1 },
    ])
  })
  it('is one marker for a flat span and none for no data', () => {
    expect(candleExtremes(['a', 'b'], [5, 5])).toEqual([{ at: 'a', value: 5 }])
    expect(candleExtremes([], [])).toEqual([])
  })
})
