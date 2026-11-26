import { describe, expect, it } from 'vitest'

import { applyPadKey, padAmount, type PadKey } from './keypad-text'

const press = (keys: PadKey[], start = '') => keys.reduce((typed, key) => applyPadKey(typed, key), start)
const digit = (value: string): PadKey => ({ kind: 'digit', value })

describe('applyPadKey', () => {
  it('types digits and one decimal mark per number', () => {
    expect(press([digit('1'), { kind: 'decimal' }, digit('5'), { kind: 'decimal' }])).toBe('1.5')
  })

  it('starts a number with 0 before the mark', () => {
    expect(press([{ kind: 'decimal' }])).toBe('0.')
  })

  it('ignores an operator first and replaces a trailing one', () => {
    expect(press([{ kind: 'operator', value: '+' }])).toBe('')
    expect(press([digit('2'), { kind: 'operator', value: '+' }, { kind: 'operator', value: '*' }])).toBe('2*')
  })

  it('allows a mark in each number of an expression', () => {
    expect(
      press([digit('1'), { kind: 'decimal' }, { kind: 'operator', value: '+' }, digit('2'), { kind: 'decimal' }]),
    ).toBe('1.+2.')
  })

  it('stops at the integer digit cap and deletes the last character', () => {
    const full = '1'.repeat(18)
    expect(applyPadKey(full, digit('2'))).toBe(full)
    expect(applyPadKey('12', { kind: 'backspace' })).toBe('1')
  })
})

describe('padAmount', () => {
  it('reads a plain number', () => {
    expect(padAmount('12.5')).toBe('12.5')
  })

  it('evaluates an expression and stays null while it is half typed', () => {
    expect(padAmount('1200+350*2')).toBe('1900')
    expect(padAmount('1200+')).toBeNull()
  })
})
