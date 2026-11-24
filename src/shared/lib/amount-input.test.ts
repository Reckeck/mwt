import { describe, expect, it } from 'vitest'

import { growsPastCap, MAX_INTEGER_DIGITS, normalizeAmount } from './amount-input'

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

describe('growsPastCap', () => {
  const over = `${'1'.repeat(MAX_INTEGER_DIGITS + 7)}.5`
  const canonical = (text: string) => normalizeAmount(text, { keepOverflow: true }).canonical

  it('accepts deleting or editing the fraction of an over-cap value', () => {
    expect(growsPastCap(canonical(over.slice(0, -2)), over, {})).toBe(false)
    expect(growsPastCap(canonical(over.replace('.5', '.')), over, {})).toBe(false)
  })

  it('refuses adding an integer digit or a fraction digit', () => {
    expect(growsPastCap(canonical(`1${over}`), over, {})).toBe(true)
    expect(growsPastCap(canonical(`${over}7`), over, {})).toBe(true)
  })

  it('accepts removing an integer digit', () => {
    expect(growsPastCap(canonical(over.slice(1)), over, {})).toBe(false)
  })
})
