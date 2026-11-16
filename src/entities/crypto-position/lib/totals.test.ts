import { describe, expect, it } from 'vitest'

import { netPercent, sumOf } from './totals'

const position = (fields: Partial<CryptoPositionView>) => fields as CryptoPositionView

describe('sumOf', () => {
  it('adds exactly what float addition gets wrong', () => {
    const positions = [position({ net_usd: '0.1' }), position({ net_usd: '0.2' })]
    expect(sumOf(positions, (item) => item.net_usd)).toEqual({ sum: '0.3', missing: 0 })
  })

  it('counts a missing figure in missing, never as zero', () => {
    const positions = [position({ net_usd: '0.1' }), position({}), position({ net_usd: '' })]
    expect(sumOf(positions, (item) => item.net_usd)).toEqual({ sum: '0.1', missing: 2 })
    expect(sumOf([position({})], (item) => item.net_usd)).toEqual({ sum: null, missing: 1 })
  })
})

describe('netPercent', () => {
  it('is the net over the put-in of the positions that have both', () => {
    const positions = [
      position({ net_usd: '10', put_in_usd: '100' }),
      position({ net_usd: '5' }),
      position({ net_usd: '10', put_in_usd: '100' }),
    ]
    expect(netPercent(positions)).toBe(10)
  })

  it('has no percent over a zero put-in', () => {
    expect(netPercent([position({ net_usd: '3', put_in_usd: '0' })])).toBeNull()
  })
})
