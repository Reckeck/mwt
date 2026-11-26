import { describe, expect, it } from 'vitest'

import { fiatRef } from '@/shared/lib/asset-ref'

import { envelopeAssets, hasEnvelopes } from './envelope-assets'

const eur = fiatRef('eur')
const usd = fiatRef('usd')

describe('envelopeAssets', () => {
  it('is the bound asset when the store has one', () => {
    expect(envelopeAssets({ binding: { asset: eur } }, [usd], 'rub')).toEqual([eur])
  })

  it('is both sides of a bound pair', () => {
    expect(envelopeAssets({ binding: { pair: { base: eur, quote: usd } } }, [], 'rub')).toEqual([eur, usd])
  })

  it('is what the store holds, else the base currency', () => {
    expect(envelopeAssets({ binding: 'any' }, [usd], 'rub')).toEqual([usd])
    expect(envelopeAssets({ binding: 'any' }, [], 'rub')).toEqual([fiatRef('rub')])
  })
})

describe('hasEnvelopes', () => {
  it('leaves out a home, points and loans', () => {
    expect(hasEnvelopes({ kind: 'property' })).toBe(false)
    expect(hasEnvelopes({ kind: 'bank' as StoreKind })).toBe(true)
  })
})
