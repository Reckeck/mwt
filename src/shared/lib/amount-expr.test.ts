import { describe, expect, it } from 'vitest'

import { evaluateAmountExpr, hasOperator, noteLine, withExpressionNote } from './amount-expr'

describe('evaluateAmountExpr', () => {
  it.each([
    ['1200+350*2', '1900'],
    ['2+3*4', '14'],
    ['(2+3)*4', '20'],
    ['10-4-3', '3'],
    ['100/8', '12.5'],
    ['100/3', '33.333333333333333333'],
    ['0.1+0.2', '0.3'],
    ['1,5+2,25', '3.75'],
    ['1 900 + 100', '2000'],
    ['1 900+100', '2000'],
    ['1\u202f900+100', '2000'],
    ['1\u00a0900\u202f000*2', '3800000'],
    ['1,234.50+1', '1235.5'],
    ['12×3', '36'],
    ['12÷4', '3'],
    ['−5+8', '3'],
    ['-5+8', '3'],
    ['2*(-3)', '-6'],
    ['((1+2))', '3'],
    ['.5+.5', '1'],
    ['1e', null],
  ])('%s -> %s', (expr, expected) => {
    const result = evaluateAmountExpr(expr)
    if (expected === null) expect(result.ok).toBe(false)
    else expect(result).toEqual({ ok: true, value: expected })
  })

  it.each([
    ['1000+10%', '1100'],
    ['1000-10%', '900'],
    ['1000*10%', '100'],
    ['1000/50%', '2000'],
    ['10%', '0.1'],
    ['200+10%+10%', '242'],
    ['1000+10%*2', '1000.2'],
    ['(100+100)-50%', '100'],
  ])('percent %s -> %s', (expr, expected) => {
    expect(evaluateAmountExpr(expr)).toEqual({ ok: true, value: expected })
  })

  it.each([
    ['500/0', 'divByZero'],
    ['5/(3-3)', 'divByZero'],
    ['', 'empty'],
    ['   ', 'empty'],
    ['1+', 'syntax'],
    ['*2', 'syntax'],
    ['(1+2', 'syntax'],
    ['1+2)', 'syntax'],
    ['1 2', 'syntax'],
    ['abc', 'syntax'],
    ['1++', 'syntax'],
    ['()', 'syntax'],
  ])('error %j -> %s', (expr, code) => {
    expect(evaluateAmountExpr(expr)).toEqual({ ok: false, code })
  })

  it('rounds half away from zero to the requested decimals', () => {
    expect(evaluateAmountExpr('2/3', { decimals: 2 })).toEqual({ ok: true, value: '0.67' })
    expect(evaluateAmountExpr('1/8', { decimals: 2 })).toEqual({ ok: true, value: '0.13' })
    expect(evaluateAmountExpr('-1/8', { decimals: 2 })).toEqual({ ok: true, value: '-0.13' })
    expect(evaluateAmountExpr('10/4', { decimals: 0 })).toEqual({ ok: true, value: '3' })
  })

  it('never throws on garbage', () => {
    for (const junk of ['((((', '1/', '%', '%%', '1%%', '9'.repeat(500) + '*' + '9'.repeat(500)]) {
      expect(() => evaluateAmountExpr(junk)).not.toThrow()
    }
  })
})

describe('hasOperator', () => {
  it.each([
    ['1200', false],
    ['1 200,50', false],
    ['1200+350', true],
    ['5-3', true],
    ['-5', false],
    ['−5', false],
    ['(5)', true],
    ['10%', true],
    ['2×3', true],
    ['', false],
  ])('%j -> %s', (text, expected) => {
    expect(hasOperator(text)).toBe(expected)
  })
})

describe('expression note', () => {
  it('formats a line', () => {
    expect(noteLine('1200+350*2', '1900')).toBe('1200+350*2 = 1900')
  })

  it('appends to a comment and keeps the manual text', () => {
    expect(withExpressionNote('lunch', null, '1+2 = 3')).toBe('lunch\n1+2 = 3')
    expect(withExpressionNote('', null, '1+2 = 3')).toBe('1+2 = 3')
  })

  it('replaces the previous line of the same field', () => {
    expect(withExpressionNote('lunch\n1+2 = 3', '1+2 = 3', '1+3 = 4')).toBe('lunch\n1+3 = 4')
    expect(withExpressionNote('1+2 = 3', '1+2 = 3', '1+3 = 4')).toBe('1+3 = 4')
  })

  it('appends when the user already removed the old line', () => {
    expect(withExpressionNote('lunch', '1+2 = 3', '1+3 = 4')).toBe('lunch\n1+3 = 4')
  })
})
