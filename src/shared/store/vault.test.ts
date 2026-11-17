import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { onVaultClosed, type VaultClosedReason } from '@/shared/lib/vault-lifecycle'

import { useVaultStore } from './vault'

vi.mock('@/shared/api/ipc', () => ({
  call: vi.fn(async (command: string) => {
    if (command === 'vault_status') return { unlocked: false }
    return undefined
  }),
}))

const status = (unlocked: boolean) => ({ unlocked }) as unknown as VaultStatus

describe('vault store closed hook', () => {
  const reasons: VaultClosedReason[] = []
  let unsubscribe = () => {}
  afterEach(() => unsubscribe())
  beforeEach(() => {
    setActivePinia(createPinia())
    reasons.length = 0
    unsubscribe = onVaultClosed((reason) => reasons.push(reason))
  })

  it('lock() emits locked', async () => {
    await useVaultStore().lock()
    expect(reasons).toEqual(['locked'])
  })

  it('applyStatus unlocked -> locked emits, unlocked -> unlocked does not', () => {
    const store = useVaultStore()
    store.applyStatus(status(true))
    store.applyStatus(status(true))
    expect(reasons).toEqual([])
    store.applyStatus(status(false))
    expect(reasons).toEqual(['locked'])
  })

  it('closeSandbox emits sandbox-exit', async () => {
    await useVaultStore().closeSandbox()
    expect(reasons).toEqual(['sandbox-exit'])
  })
})
