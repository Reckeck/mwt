import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'
import { emitVaultClosed } from '@/shared/lib/vault-lifecycle'
import { useSettingsStore } from '@/shared/store/settings'
import { useTradeLookupStore } from '@/shared/store/tradeLookup'

import { queryClient } from './providers/query'
import { registerVaultClosedSubscribers } from './vault-closed-subscribers'

let stop: () => void

beforeEach(() => {
  setActivePinia(createPinia())
  stop = registerVaultClosedSubscribers()
})
afterEach(() => {
  stop()
  queryClient.clear()
})

describe('vault closed subscribers', () => {
  it('empties the query cache, vault settings and vault-bound stores', () => {
    queryClient.setQueryData(['x'], 1)
    const settings = useSettingsStore()
    settings.vault = { display_decimals: 2 } as Settings
    const lookup = useTradeLookupStore()
    lookup.executions = [{} as BybitExecution]
    emitVaultClosed('locked')
    expect(queryClient.getQueryData(['x'])).toBeUndefined()
    expect(queryClient.getQueryCache().getAll()).toHaveLength(0)
    expect(settings.vault).toBeNull()
    expect(lookup.executions).toEqual([])
  })

  it('discards a loadVault response that arrives after the vault was closed', async () => {
    let resolveA!: (value: Settings) => void
    vi.mocked(call).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveA = resolve as never
        }),
    )
    const settings = useSettingsStore()
    const pending = settings.loadVault()
    emitVaultClosed('locked')
    resolveA({ display_decimals: 3 } as Settings)
    await pending
    expect(settings.vault).toBeNull()
  })
})
