import { describe, expect, it } from 'vitest'

import { byUsage, categoryUsage, frequentCategory } from './frequent-category'

const payment = (...categories: string[]) =>
  ({ payment: { meta: { id: 'x', categories }, fee: null } }) as unknown as Transaction
const topup = (...categories: string[]) =>
  ({ topup: { meta: { id: 'y', categories }, fee: null } }) as unknown as Transaction

describe('frequentCategory', () => {
  it('is the category used most by this kind', () => {
    const list = [payment('taxi'), payment('food'), payment('food'), topup('salary'), topup('salary'), topup('salary')]
    expect(frequentCategory(list, 'payment')).toBe('food')
    expect(frequentCategory(list, 'topup')).toBe('salary')
  })
  it('gives a tie to the one used last (the list is newest first)', () => {
    expect(frequentCategory([payment('taxi'), payment('food'), payment('food'), payment('taxi')], 'payment')).toBe(
      'taxi',
    )
  })
  it('is null without history', () => {
    expect(frequentCategory([], 'payment')).toBeNull()
    expect(frequentCategory([topup('salary')], 'payment')).toBeNull()
  })
})

describe('byUsage', () => {
  const options = ['a', 'b', 'c', 'd'].map((value) => ({ value }))
  it('puts the more used first and keeps the order of the unused', () => {
    const usage = categoryUsage([payment('c'), payment('d'), payment('d')], 'payment')
    expect(byUsage(options, usage).map((option) => option.value)).toEqual(['d', 'c', 'a', 'b'])
  })
  it('keeps the order without history', () => {
    expect(byUsage(options, new Map()).map((option) => option.value)).toEqual(['a', 'b', 'c', 'd'])
  })
})
