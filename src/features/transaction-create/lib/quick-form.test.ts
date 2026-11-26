import { describe, expect, it } from 'vitest'

import { emptyQuickForm, nextInSeries, quickAssetStoreId, quickErrors, quickFormToInput } from './quick-form'

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

  it('wants a destination wallet and a received amount for an exchange', () => {
    const form = { ...emptyQuickForm('convert', 'a'), amount: '100' }
    const errors = quickErrors(form)
    expect(errors.category).toBeNull()
    expect(errors.toStoreId).toBeTruthy()
    expect(errors.toAmount).toBeTruthy()
    expect(Object.values(quickErrors({ ...form, toStoreId: 'b', toAmount: '92' })).some(Boolean)).toBe(false)
  })

  it('refuses an exchange of a wallet for another of the same asset', () => {
    const form = { ...emptyQuickForm('convert', 'a'), toStoreId: 'b', amount: '100', toAmount: '100' }
    const usd = { ...rub, symbol: 'USD', quote: 'USD' }
    expect(quickErrors(form, { asset: rub, toAsset: rub }).toStoreId).toBeTruthy()
    expect(quickErrors(form, { asset: rub, toAsset: usd }).toStoreId).toBeNull()
    expect(quickErrors(form, { asset: rub, toAsset: null }).toStoreId).toBeNull()
    expect(quickErrors({ ...form, kind: 'transfer' }, { asset: rub, toAsset: rub }).toStoreId).toBeNull()
  })

  it('lets a transfer go to or come from a contact instead of a wallet', () => {
    const out = { ...emptyQuickForm('transfer', 'a'), amount: '5', toContactId: 'c1' }
    expect(quickErrors(out).toStoreId).toBeNull()
    const back = { ...emptyQuickForm('transfer', ''), amount: '5', toStoreId: 'b', fromContactId: 'c1' }
    expect(quickErrors(back).storeId).toBeNull()
    expect(quickAssetStoreId(back)).toBe('b')
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

describe('quickFormToInput contacts', () => {
  it('writes a contact side of a transfer', () => {
    const form = { ...emptyQuickForm('transfer', 's1'), amount: '10', toContactId: 'c1' }
    expect(quickFormToInput(form, rub)).toMatchObject({
      from: { store: 's1' },
      to: { contact: { contact_id: 'c1', account_id: null } },
    })
  })
})

describe('nextInSeries', () => {
  it('keeps everything but the amount', () => {
    const form = { ...emptyQuickForm('payment', 's1'), amount: '10', category: 'food', timestamp: '2026-01-01T10:00' }
    expect(nextInSeries(form)).toEqual({ ...form, amount: '' })
  })
})

describe('quickFormToInput counterparty and note', () => {
  it('carries the contact and the trimmed note of an expense', () => {
    const form = { ...emptyQuickForm('payment', 'a'), category: 'food', amount: '5', contactId: 'c1', note: ' lunch ' }
    const input = quickFormToInput(form, rub)
    expect(input.common.contact_id).toBe('c1')
    expect(input.common.comment).toBe('lunch')
  })

  it('sends no contact when none is picked', () => {
    expect(quickFormToInput({ ...emptyQuickForm('payment', 'a'), amount: '5' }, rub).common.contact_id).toBeNull()
  })
})

describe('quickFormToInput for an exchange', () => {
  it("gives from the first wallet and gets in the second wallet's asset", () => {
    const usd: AssetRef = { ...rub, symbol: 'USD', name: 'USD', quote: 'USD' }
    const form = { ...emptyQuickForm('convert', 'a'), toStoreId: 'b', amount: '100', toAmount: '92' }
    const input = quickFormToInput(form, rub, usd)
    if (input.type !== 'convert') throw new Error('not an exchange')
    expect(input.from).toMatchObject({ account: { store: 'a' }, amount: '100', asset: { symbol: 'RUB' } })
    expect(input.to).toMatchObject({ account: { store: 'b' }, amount: '92', asset: { symbol: 'USD' } })
  })
})
