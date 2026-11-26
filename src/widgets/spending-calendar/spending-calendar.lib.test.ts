import { describe, expect, it } from 'vitest'

import { heatLevel } from './spending-calendar.lib'

describe('heatLevel', () => {
  it('is 0 for a day without spending or a month without a maximum', () => {
    expect(heatLevel(0, 100)).toBe(0)
    expect(heatLevel(50, 0)).toBe(0)
  })
  it('puts the heaviest day on the top step', () => {
    expect(heatLevel(100, 100)).toBe(4)
  })
  it('keeps a small spend off step 0', () => {
    expect(heatLevel(1, 1000)).toBe(1)
  })
  it('splits the maximum into equal shares', () => {
    expect(heatLevel(50, 100)).toBe(2)
    expect(heatLevel(75, 100)).toBe(3)
  })
})
