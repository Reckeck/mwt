import { describe, expect, it } from 'vitest'

import { decimalAdd, decimalCmp, decimalInput, decimalMul, decimalShare, decimalSub, parseDecimal } from './decimal'

describe('parseDecimal', () => {
  it('accepts a comma as the decimal mark and trims', () => {
    expect(parseDecimal(' 12,50 ')).toBe('12.50')
    expect(parseDecimal('-3')).toBe('-3')
  })

  it('rejects what is not a plain number', () => {
    expect(parseDecimal('')).toBeNull()
    expect(parseDecimal('1e3')).toBeNull()
    expect(parseDecimal('1.')).toBeNull()
    expect(parseDecimal('abc')).toBeNull()
  })
})

describe('decimal arithmetic', () => {
  it('does not drift like floats', () => {
    expect(decimalAdd('0.1', '0.2')).toBe('0.3')
    expect(decimalSub('0.3', '0.1')).toBe('0.2')
  })

  it('crosses zero and drops trailing zeros', () => {
    expect(decimalSub('1', '1.50')).toBe('-0.5')
    expect(decimalSub('2.50', '2.5')).toBe('0')
    expect(decimalAdd('-1', '1')).toBe('0')
  })

  it('multiplies exactly', () => {
    expect(decimalMul('1.1', '1.1')).toBe('1.21')
    expect(decimalMul('-2', '0.5')).toBe('-1')
  })

  it('returns null for non-numbers', () => {
    expect(decimalAdd('x', '1')).toBeNull()
    expect(decimalSub('1', '')).toBeNull()
    expect(decimalMul('1', 'y')).toBeNull()
  })
})

describe('decimalInput', () => {
  it('rounds to the given places without trailing zeros', () => {
    expect(decimalInput(1.23456, 2)).toBe('1.23')
    expect(decimalInput(5, 4)).toBe('5')
  })

  it('is empty for non-positive or non-finite numbers', () => {
    expect(decimalInput(0, 2)).toBe('')
    expect(decimalInput(-1, 2)).toBe('')
    expect(decimalInput(Number.NaN, 2)).toBe('')
  })
})

describe('decimalCmp', () => {
  it('compares exactly', () => {
    expect(decimalCmp('0.1', '0.10')).toBe(0)
    expect(decimalCmp('0.30000000000000004', '0.3')).toBe(1)
    expect(decimalCmp('-1', '0.5')).toBe(-1)
  })

  it('is null for a non-number', () => {
    expect(decimalCmp('x', '1')).toBeNull()
  })
})

describe('decimalShare', () => {
  it('splits a given amount by parts exactly, rounding half away from zero', () => {
    expect(decimalShare('100', '3000', '9000', 2)).toBe('33.33')
    expect(decimalShare('100', '6000', '9000', 2)).toBe('66.67')
    expect(decimalShare('0.3', '1', '3', 1)).toBe('0.1')
    expect(decimalShare('1', '1', '8', 2)).toBe('0.13')
    expect(decimalShare('-1', '1', '8', 2)).toBe('-0.13')
  })

  it('is null for a non-number or a non-positive total', () => {
    expect(decimalShare('100', '1', '0', 2)).toBeNull()
    expect(decimalShare('100', '1', '-3', 2)).toBeNull()
    expect(decimalShare('x', '1', '3', 2)).toBeNull()
  })
})
