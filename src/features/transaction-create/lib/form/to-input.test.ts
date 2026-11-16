import { describe, expect, it } from 'vitest'

import { emptyForm } from './form-state'
import { pricedAssetKey } from './to-input'

const asset = (symbol: string): AssetRef =>
  ({ symbol, name: symbol, coingecko_id: null, chain: null, contract: null }) as AssetRef

describe('pricedAssetKey', () => {
  it('is equal for a copy of one asset and differs between assets', () => {
    const first = { ...emptyForm('s1', 'payment'), asset: asset('USD') }
    const copy = { ...emptyForm('s1', 'payment'), asset: { ...asset('USD') } }
    const other = { ...emptyForm('s1', 'payment'), asset: asset('RUB') }
    expect(pricedAssetKey(first)).toBe(pricedAssetKey(copy))
    expect(pricedAssetKey(first)).not.toBe(pricedAssetKey(other))
  })

  it('is empty without an asset', () => {
    expect(pricedAssetKey(emptyForm('s1', 'payment'))).toBe('')
  })
})
