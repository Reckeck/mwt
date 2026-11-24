import { describe, expect, it } from 'vitest'

import { flowAmount, flowOfAmount } from './money-flow'

describe('money flow (D7)', () => {
  it('reads the direction of a signed figure', () => {
    expect(flowOfAmount('-12.5')).toBe('expense')
    expect(flowOfAmount('40')).toBe('income')
    expect(flowOfAmount('0')).toBe('move')
    expect(flowOfAmount(null)).toBe('move')
  })

  it('forces the sign: minus for an expense, plain magnitude otherwise', () => {
    expect(flowAmount('12.5', 'expense')).toBe(-12.5)
    expect(flowAmount('-40', 'income')).toBe(40)
    expect(flowAmount('-7', 'move')).toBe(7)
    expect(flowAmount('x', 'move')).toBeNull()
  })
})
