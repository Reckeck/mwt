import { describe, expect, it } from 'vitest'

import { emptyQuickForm, quickFormToInput } from '@/features/transaction-create'

import { applyMore, emptyQuickMore, filledMoreCount, moreError } from './quick-more'

const rub: AssetRef = { symbol: 'RUB', name: 'RUB', coingecko_id: null, chain: 'fiat', contract: null, quote: 'RUB' }
const base = (kind: 'payment' | 'transfer' | 'convert') =>
  quickFormToInput(
    { ...emptyQuickForm(kind, 'a'), toStoreId: 'b', amount: '10', toAmount: '9', category: 'food' },
    rub,
    rub,
  )

describe('applyMore', () => {
  it('changes nothing when nothing is filled', () => {
    expect(applyMore(base('payment'), emptyQuickMore(), rub, 'payment')).toMatchObject({ fee: null, to: null })
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
    const input = applyMore(base('payment'), more, rub, 'payment')
    if (input.type !== 'payment') throw new Error('kind')
    expect(input.fee).toMatchObject({ amount: '1.5', asset: { symbol: 'RUB' } })
    expect(input.to).toEqual({ raw: 'Cafe' })
    expect(input.common.property_id).toBe('home')
    expect(input.details).toMatchObject({ network: 'ethereum', tx_hash: '0xabc' })
  })

  it('marks a transfer as a loan and leaves the payee out', () => {
    const more = { ...emptyQuickMore(), asDebt: true, payee: 'ignored' }
    const input = applyMore(base('transfer'), more, rub, 'transfer')
    if (input.type !== 'transfer') throw new Error('kind')
    expect(input.debt).toBe('lend')
  })

  it('keeps an exchange to a fee and the property', () => {
    const input = applyMore(base('convert'), { ...emptyQuickMore(), fee: '2', asDebt: true }, rub, 'convert')
    expect(input).toMatchObject({ type: 'convert', fee: { amount: '2' } })
  })
})

describe('filledMoreCount and moreError', () => {
  it('counts only the fields of the kind', () => {
    const more = { ...emptyQuickMore(), fee: '1', payee: 'x', asDebt: true }
    expect(filledMoreCount(more, 'payment')).toBe(2)
    expect(filledMoreCount(more, 'transfer')).toBe(2)
  })
  it('refuses a fee that is not a positive amount', () => {
    expect(moreError({ ...emptyQuickMore(), fee: '0' })).toBeTruthy()
    expect(moreError({ ...emptyQuickMore(), fee: '' })).toBeNull()
  })
})
