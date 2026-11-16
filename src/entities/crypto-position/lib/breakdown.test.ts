import { describe, expect, it } from 'vitest'

import { isComplete, knownSum, positionBreakdown, withdrawnUsd } from './breakdown'

const base = {
  id: 'x',
  account_id: 'a',
  chain: 'ethereum',
  protocol: 'uniswap-v3',
  name: 'x',
  kind: 'pool',
  status: 'open',
  assets: [],
  approximate: false,
  updated_at: '2026-11-16T00:00:00Z',
  tracking: { position: { protocol: 'uniswap-v3', chain: 'ethereum', id: '1' } },
} as CryptoPositionView

describe('positionBreakdown', () => {
  it('adds a pool up to the engine net, with no accrual line and the IL left out', () => {
    const position: CryptoPositionView = {
      ...base,
      invested_usd: '4000',
      value_usd: '5656.8542',
      fees_collected_usd: '120',
      fees_uncollected_usd: '30',
      il_usd: '-343.1458',
      gas_usd: '18.5',
      net_usd: '1788.3542',
    }
    const lines = positionBreakdown(position)
    expect(lines.map((line) => line.id)).toEqual(['priceChange', 'feesCollected', 'feesUncollected', 'gas'])
    expect(knownSum(lines)).toBe('1788.3542')
    expect(isComplete(lines)).toBe(true)
  })

  it('splits a supply into the accrual and the price change without counting the accrual twice', () => {
    const position: CryptoPositionView = {
      ...base,
      kind: 'lending',
      protocol: 'aave-v3',
      invested_usd: '5000',
      value_usd: '6000',
      accrued_usd: '240',
      gas_usd: '12.4',
      net_usd: '987.6',
    }
    const lines = positionBreakdown(position)
    expect(lines.find((line) => line.id === 'priceChange')?.amount).toBe('760')
    expect(knownSum(lines)).toBe('987.6')
  })

  it('keeps a missing figure as null, never zero', () => {
    const lines = positionBreakdown({ ...base, invested_usd: '1900', gas_usd: '21.4' })
    expect(lines.find((line) => line.id === 'priceChange')).toMatchObject({ amount: null, reason: 'notRead' })
    expect(lines.find((line) => line.id === 'feesCollected')?.amount).toBeNull()
    expect(isComplete(lines)).toBe(false)
  })

  it('reads a closed pool whose withdrawals exceed the deposits', () => {
    const position: CryptoPositionView = {
      ...base,
      status: 'closed',
      put_in_usd: '1000',
      invested_usd: '-310.5',
      value_usd: '0',
      fees_collected_usd: '86.4',
      fees_uncollected_usd: '0',
      gas_usd: '21.9',
      net_usd: '375',
    }
    expect(knownSum(positionBreakdown(position))).toBe('375')
    expect(withdrawnUsd(position)).toBe('1310.5')
  })

  it('prints the claimed rewards of a staking position so the lines add up to the engine net exactly', () => {
    const position: CryptoPositionView = {
      ...base,
      kind: 'staking',
      protocol: 'lido',
      invested_usd: '0.1',
      value_usd: '0.3',
      accrued_usd: '0.2',
      fees_collected_usd: '0.1',
      fees_uncollected_usd: '0',
      gas_usd: '0.1',
      net_usd: '0.2',
    }
    const lines = positionBreakdown(position)
    expect(lines.map((line) => line.id)).toEqual(['accrual', 'priceChange', 'feesCollected', 'feesUncollected', 'gas'])
    expect(knownSum(lines)).toBe(position.net_usd)
  })

  it('keeps the not-applicable fees line for a lending position with no fee figures', () => {
    const lines = positionBreakdown({
      ...base,
      kind: 'lending',
      protocol: 'aave-v3',
      invested_usd: '5000',
      value_usd: '6000',
      accrued_usd: '240',
      gas_usd: '12.4',
    })
    expect(lines.filter((line) => line.id === 'fees')).toEqual([
      { id: 'fees', amount: null, approximate: false, reason: 'notApplicable' },
    ])
    expect(isComplete(lines)).toBe(true)
  })

  it('keeps the not-applicable fees line when both fee figures are exact zeros', () => {
    const lines = positionBreakdown({
      ...base,
      kind: 'staking',
      fees_collected_usd: '0',
      fees_uncollected_usd: '0.00',
    })
    expect(lines.map((line) => line.id)).toContain('fees')
  })

  it('reads an unknown fee figure beside a known one as not read, never zero', () => {
    const lines = positionBreakdown({ ...base, kind: 'staking', fees_collected_usd: '5' })
    expect(lines.find((line) => line.id === 'feesUncollected')).toMatchObject({ amount: null, reason: 'notRead' })
  })
})
