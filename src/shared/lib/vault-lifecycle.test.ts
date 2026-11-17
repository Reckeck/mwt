import { describe, expect, it, vi } from 'vitest'

import { emitVaultClosed, onVaultClosed } from './vault-lifecycle'

describe('vault-lifecycle', () => {
  it('runs subscribers with the reason', () => {
    const first = vi.fn()
    const second = vi.fn()
    const offFirst = onVaultClosed(first)
    const offSecond = onVaultClosed(second)
    emitVaultClosed('switched')
    expect(first).toHaveBeenCalledWith('switched')
    expect(second).toHaveBeenCalledWith('switched')
    offFirst()
    offSecond()
  })

  it('stops calling an unsubscribed callback', () => {
    const callback = vi.fn()
    const off = onVaultClosed(callback)
    off()
    emitVaultClosed('locked')
    expect(callback).not.toHaveBeenCalled()
  })
})
