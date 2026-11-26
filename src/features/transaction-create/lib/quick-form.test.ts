import { describe, expect, it } from 'vitest'

import { emptyQuickForm, nextInSeries, quickErrors, quickFormToInput } from './quick-form'

const rub: AssetRef = { symbol: 'RUB', name: 'RUB', coingecko_id: null, chain: 'fiat', contract: null, quote: 'RUB' }

describe('quickErrors', () => {
  it('names every missing field of an empty expense', () => {
    const errors = quickErrors(emptyQuickForm('payment'))
    expect(errors.amount).toBeTruthy()
    expect(errors.storeId).toBeTruthy()
    expect(errors.category).toBeTruthy()
    expect(errors.toStoreId).toBeNull()
  })

  it('wants a destination for a transfer, not a category, and not the same wallet', () => {
    const form = { ...emptyQuickForm('transfer', 'a'), amount: '5' }
    expect(quickErrors(form).category).toBeNull()
    expect(quickErrors(form).toStoreId).toBeTruthy()
    expect(quickErrors({ ...form, toStoreId: 'a' }).toStoreId).toBeTruthy()
    expect(quickErrors({ ...form, toStoreId: 'b' }).toStoreId).toBeNull()
  })

  it('refuses a zero amount and a date out of range', () => {
    const form = { ...emptyQuickForm('payment', 'a'), category: 'food', amount: '0', timestamp: '1800-01-01T10:00' }
    expect(quickErrors(form).amount).toBeTruthy()
    expect(quickErrors(form).timestamp).toBeTruthy()
    expect(Object.values(quickErrors({ ...form, amount: '1.5', timestamp: '' })).some(Boolean)).toBe(false)
  })
})

describe('quickFormToInput', () => {
  it('builds each kind', () => {
    const base = { ...emptyQuickForm('payment', 's1'), amount: '10', category: 'food' }
    expect(quickFormToInput(base, rub)).toMatchObject({
      type: 'payment',
      store_id: 's1',
      amount: '10',
      common: { categories: ['food'] },
    })
    expect(quickFormToInput({ ...base, kind: 'topup' }, rub)).toMatchObject({ type: 'topup', store_id: 's1' })
    expect(quickFormToInput({ ...base, kind: 'transfer', toStoreId: 's2' }, rub)).toMatchObject({
      type: 'transfer',
      from: { store: 's1' },
      to: { store: 's2' },
      common: { categories: [] },
    })
  })
})

describe('nextInSeries', () => {
  it('keeps everything but the amount', () => {
    const form = { ...emptyQuickForm('payment', 's1'), amount: '10', category: 'food', timestamp: '2026-01-01T10:00' }
    expect(nextInSeries(form)).toEqual({ ...form, amount: '' })
  })
})
