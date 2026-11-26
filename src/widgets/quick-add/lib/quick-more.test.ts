import { describe, expect, it } from 'vitest'

import { emptyQuickForm, quickFormToInput } from '@/features/transaction-create'

import { applyMore, emptyQuickMore, filledMoreCount, moreError, moreFieldsOf } from './quick-more'

const rub: AssetRef = { symbol: 'RUB', name: 'RUB', coingecko_id: null, chain: 'fiat', contract: null, quote: 'RUB' }
const formOf = (kind: 'payment' | 'transfer' | 'convert') => ({
  ...emptyQuickForm(kind, 'a'),
  toStoreId: 'b',
  amount: '10',
  toAmount: '9',
  category: 'food',
})
const base = (kind: 'payment' | 'transfer' | 'convert') => quickFormToInput(formOf(kind), rub, rub)

describe('applyMore', () => {
  it('changes nothing when nothing is filled', () => {
    expect(applyMore(base('payment'), emptyQuickMore(), rub, formOf('payment'))).toMatchObject({ fee: null, to: null })
  })

  it('puts the fee, the payee, the property and the hash on an expense', () => {
    const more = {
      ...emptyQuickMore(),
      fee: '1.5',
      payee: 'Cafe',
      propertyId: 'home',
      network: 'ethereum',
      txHash: ' 0xabc ',
    }
    const input = applyMore(base('payment'), more, rub, formOf('payment'))
    if (input.type !== 'payment') throw new Error('kind')
    expect(input.fee).toMatchObject({ amount: '1.5', asset: { symbol: 'RUB' } })
    expect(input.to).toEqual({ raw: 'Cafe' })
    expect(input.common.property_id).toBe('home')
    expect(input.details).toMatchObject({ network: 'ethereum', tx_hash: '0xabc' })
  })

  it('marks a transfer to a contact as a loan and leaves the payee out', () => {
    const more = { ...emptyQuickMore(), asDebt: true, payee: 'ignored' }
    const form = { ...emptyQuickForm('transfer', 'a'), amount: '10', toContactId: 'c1' }
    const input = applyMore(quickFormToInput(form, rub), more, rub, form)
    if (input.type !== 'transfer') throw new Error('kind')
    expect(input.debt).toBe('lend')
  })

  it('marks a transfer from a contact as borrowed', () => {
    const form = { ...emptyQuickForm('transfer', ''), toStoreId: 'b', amount: '10', fromContactId: 'c1' }
    const input = applyMore(quickFormToInput(form, rub), { ...emptyQuickMore(), asDebt: true }, rub, form)
    if (input.type !== 'transfer') throw new Error('kind')
    expect(input.debt).toBe('borrow')
  })

  it('leaves debt null for a wallet-to-wallet transfer', () => {
    const input = applyMore(base('transfer'), { ...emptyQuickMore(), asDebt: true }, rub, formOf('transfer'))
    if (input.type !== 'transfer') throw new Error('kind')
    expect(input.debt).toBeNull()
  })

  it('keeps an exchange to a fee and the property', () => {
    const input = applyMore(base('convert'), { ...emptyQuickMore(), fee: '2', asDebt: true }, rub, formOf('convert'))
    expect(input).toMatchObject({ type: 'convert', fee: { amount: '2' } })
  })
})

describe('moreFieldsOf', () => {
  it('offers the event on an expense and the loan only beside a contact', () => {
    expect(moreFieldsOf('payment')).toContain('event')
    expect(moreFieldsOf('topup')).not.toContain('event')
    expect(moreFieldsOf('transfer')).not.toContain('debt')
    expect(moreFieldsOf('transfer', true)).toContain('debt')
  })
})

describe('filledMoreCount and moreError', () => {
  it('counts only the fields of the kind', () => {
    const more = { ...emptyQuickMore(), fee: '1', payee: 'x', asDebt: true }
    expect(filledMoreCount(more, 'payment')).toBe(2)
    expect(filledMoreCount(more, 'transfer')).toBe(1)
    expect(filledMoreCount(more, 'transfer', true)).toBe(2)
  })
  it('refuses a fee that is not a positive amount', () => {
    expect(moreError({ ...emptyQuickMore(), fee: '0' })).toBeTruthy()
    expect(moreError({ ...emptyQuickMore(), fee: '' })).toBeNull()
  })
})
