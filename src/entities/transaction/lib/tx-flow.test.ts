import { describe, expect, it } from 'vitest'

import { txLegFlow } from './tx-flow'

describe('txLegFlow', () => {
  it('mutes a move between own wallets and an exchange', () => {
    expect(txLegFlow('transfer', true, '100')).toBe('move')
    expect(txLegFlow('convert', false, '-100')).toBe('move')
  })

  it('keeps the sign of a transfer to an outside party', () => {
    expect(txLegFlow('transfer', false, '-100')).toBe('expense')
    expect(txLegFlow('transfer', false, '100')).toBe('income')
  })

  it('keeps the direction of a move in one wallet history', () => {
    expect(txLegFlow('transfer', true, '-100', true)).toBe('expense')
    expect(txLegFlow('convert', false, '100', true)).toBe('income')
  })
})
