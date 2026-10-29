import { describe, expect, it } from 'vitest'

import { checkUnits, splitCaption, splitMode, splitOf, unitsTotal } from './split'

describe('splitMode', () => {
  it('reads an empty circle as everybody', () => {
    expect(splitMode({ equal: { among: [] } })).toBe('all')
    expect(splitMode({ equal: { among: ['a'] } })).toBe('some')
    expect(splitMode({ by_units: { units: { a: '3' } } })).toBe('units')
  })
})

describe('splitOf', () => {
  it('drops zero and blank units and keeps the circle only for «some»', () => {
    expect(splitOf('units', [], { a: '3', b: '0', c: '', d: '5' })).toEqual({ by_units: { units: { a: '3', d: '5' } } })
    expect(splitOf('all', ['a'], {})).toEqual({ equal: { among: [] } })
    expect(splitOf('some', ['a'], {})).toEqual({ equal: { among: ['a'] } })
  })
})

describe('checkUnits', () => {
  it('accepts units that add up to the quantity bought', () => {
    expect(unitsTotal({ a: '3', b: '5' })).toBe('8')
    expect(checkUnits({ a: '3', b: '5' }, '8')).toEqual({ kind: 'ok' })
  })

  it('says how many are left or too many', () => {
    expect(checkUnits({ a: '3', b: '4' }, '8')).toEqual({ kind: 'short', left: '1' })
    expect(checkUnits({ a: '5', b: '5' }, '8')).toEqual({ kind: 'over', extra: '2' })
    expect(checkUnits({ a: '', b: '0' }, '8')).toEqual({ kind: 'empty' })
  })

  it('accepts a comma as the decimal mark', () => {
    expect(checkUnits({ a: '1,5', b: '1' }, '2.5')).toEqual({ kind: 'ok' })
  })
})

describe('splitCaption', () => {
  const names: Record<string, string> = { a: 'Петя', b: 'Вася' }
  const nameOf = (id: string) => names[id] ?? id
  it('names the chosen people and the units', () => {
    expect(splitCaption({ equal: { among: ['a', 'b'] } }, nameOf)).toEqual({ mode: 'some', parts: ['Петя', 'Вася'] })
    expect(splitCaption({ by_units: { units: { a: '3', b: '5' } } }, nameOf)).toEqual({
      mode: 'units',
      parts: ['Петя 3', 'Вася 5'],
    })
    expect(splitCaption({ equal: { among: [] } }, nameOf).mode).toBe('all')
  })
})
