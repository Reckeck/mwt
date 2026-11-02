import { describe, expect, it } from 'vitest'

import { describeField, fieldLabel, humanizeField } from './fields'

const t = (key: string) => `T(${key})`

describe('humanizeField', () => {
  it('turns dotted and snake case names into words', () => {
    expect(humanizeField('meta.comment')).toBe('Meta comment')
    expect(humanizeField('target_amount')).toBe('Target amount')
    expect(humanizeField('')).toBe('')
  })
})

describe('fieldLabel', () => {
  it('uses the translation when there is one', () => {
    expect(fieldLabel('amount', t, () => true)).toBe('T(sync.field.amount)')
  })

  it('humanizes the name when there is none', () => {
    expect(fieldLabel('unit_price', t, () => false)).toBe('Unit price')
  })
})

describe('describeField', () => {
  it('carries the sides and the currency for Money', () => {
    const view = describeField({ field: 'amount', before: '10', after: '25', money_currency: 'usd' }, t, () => false)
    expect(view).toEqual({ label: 'Amount', before: '10', after: '25', currency: 'usd' })
  })
})
