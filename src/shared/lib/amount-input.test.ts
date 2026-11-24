import { describe, expect, it } from 'vitest'

import { MAX_INTEGER_DIGITS, normalizeAmount } from './amount-input'

describe('normalizeAmount digit cap', () => {
  it('accepts exactly the cap and flags one more', () => {
    const exact = '9'.repeat(MAX_INTEGER_DIGITS)
    expect(normalizeAmount(exact).overflow).toBeUndefined()
    expect(normalizeAmount(`${exact}9`).overflow).toBe(true)
  })

  it('does not count fraction digits', () => {
    expect(normalizeAmount(`${'1'.repeat(MAX_INTEGER_DIGITS)},${'5'.repeat(30)}`).overflow).toBeUndefined()
  })

  it('handles a 5000-digit paste in under 50 ms', () => {
    const started = performance.now()
    const result = normalizeAmount('7'.repeat(5000))
    expect(performance.now() - started).toBeLessThan(50)
    expect(result.overflow).toBe(true)
    expect(result.canonical).toHaveLength(MAX_INTEGER_DIGITS)
  })
})
