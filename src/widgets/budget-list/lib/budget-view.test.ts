import { describe, expect, it } from 'vitest'

import { budgetBar, daysLeft } from './budget-view'

describe('budgetBar', () => {
  it('is plain under the warning share', () => {
    expect(budgetBar(0.4, true)).toEqual({ value: 40, tone: 'primary' })
  })

  it('turns amber from the warning share only when the limit warns', () => {
    expect(budgetBar(0.95, true).tone).toBe('warning')
    expect(budgetBar(0.95, false).tone).toBe('primary')
  })

  it('is full and red at the limit', () => {
    expect(budgetBar(1, true)).toEqual({ value: 100, tone: 'destructive' })
  })

  it('puts the limit tick on the track once spending passed it', () => {
    const bar = budgetBar(1.25, false)
    expect(bar.tone).toBe('destructive')
    expect(bar.value).toBe(100)
    expect(bar.marker).toBeCloseTo(80)
  })

  it('reads nothing as empty', () => {
    expect(budgetBar(Number.NaN, true)).toEqual({ value: 0, tone: 'primary' })
  })
})

describe('daysLeft', () => {
  it('counts whole days and never less than one', () => {
    const now = Date.parse('2026-11-26T10:00:00Z')
    expect(daysLeft('2026-11-30T00:00:00Z', now)).toBe(4)
    expect(daysLeft('2026-11-01T00:00:00Z', now)).toBe(1)
  })
})
