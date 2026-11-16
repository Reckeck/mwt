import { describe, expect, it } from 'vitest'

import { feesFigure } from './row-figures'

const base = {
  id: 'x',
  account_id: 'a',
  chain: 'ethereum',
  protocol: 'lido',
  name: 'x',
  kind: 'staking',
  status: 'open',
  assets: [],
  approximate: false,
  updated_at: '2026-11-16T00:00:00Z',
  tracking: { position: { protocol: 'lido', chain: 'ethereum', id: '1' } },
} as CryptoPositionView

describe('feesFigure', () => {
  it('sums the two fee figures exactly, where float addition would not', () => {
    const figure = feesFigure({ ...base, kind: 'pool', fees_collected_usd: '0.1', fees_uncollected_usd: '0.2' })
    expect(figure).toEqual({ amount: '0.3' })
  })

  it('shows the claimed rewards of a stake, the same rule the breakdown follows', () => {
    const figure = feesFigure({ ...base, fees_collected_usd: '12.5', fees_uncollected_usd: '0' })
    expect(figure).toEqual({ amount: '12.5' })
  })

  it('leaves a stake with nothing claimed or pending as not applicable, never a zero', () => {
    expect(feesFigure(base)).toEqual({ amount: null, reason: 'notApplicable' })
    expect(feesFigure({ ...base, fees_collected_usd: '0', fees_uncollected_usd: '0.00' })).toEqual({
      amount: null,
      reason: 'notApplicable',
    })
  })

  it('reads a pool whose fees nobody could read as not read, never a zero', () => {
    expect(feesFigure({ ...base, kind: 'pool' })).toEqual({ amount: null, reason: 'notRead' })
  })
})
