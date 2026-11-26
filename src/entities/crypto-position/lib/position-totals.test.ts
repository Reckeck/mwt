import { describe, expect, it } from 'vitest'

import { positionTotals, protocolsOf } from './position-totals'

const position = (patch: Partial<CryptoPositionView>) =>
  ({ id: 'a', protocol: 'lido', status: 'open', value_usd: '100', ...patch }) as CryptoPositionView

describe('positionTotals', () => {
  it('sums the open positions with a known worth and counts the rest apart', () => {
    const totals = positionTotals([
      position({ value_usd: '100.5' }),
      position({ value_usd: undefined }),
      position({ status: 'closed', value_usd: '0' }),
    ])
    expect(totals).toEqual({ value: 100.5, missing: 1 })
  })
})

describe('protocolsOf', () => {
  it('lists each protocol once', () => {
    expect(
      protocolsOf([
        position({ protocol: 'aave-v3' }),
        position({ protocol: 'lido' }),
        position({ protocol: 'aave-v3' }),
      ]),
    ).toEqual(['aave-v3', 'lido'])
  })
})
