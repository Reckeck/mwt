import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { CRYPTO_ACCOUNTS_KEY, useCryptoSyncStatus } from '@/entities/crypto-account'
import { call } from '@/shared/api/ipc'

import { useTrackingConsent } from './use-crypto-network'

describe('the consent cache', () => {
  beforeEach(() => vi.mocked(call).mockReset())

  it('is the same entry the connect dialog refreshes, so Settings never lags behind a consent given there', async () => {
    let consent: boolean | null = null
    vi.mocked(call).mockImplementation(async () => ({ consent }) as CryptoSyncStatus)
    const queryClient = new QueryClient()
    let settings!: ReturnType<typeof useTrackingConsent>
    let dialog!: ReturnType<typeof useCryptoSyncStatus>
    mount(
      defineComponent({
        setup() {
          settings = useTrackingConsent()
          dialog = useCryptoSyncStatus()
          return () => h('div')
        },
      }),
      { global: { plugins: [[VueQueryPlugin, { queryClient }]] } },
    )
    await flushPromises()
    expect(settings.data.value).toBeNull()
    expect(dialog.data.value?.consent).toBeNull()

    // What the connect dialog does after the consent is saved.
    consent = true
    await queryClient.invalidateQueries({ queryKey: [...CRYPTO_ACCOUNTS_KEY, 'sync'] })
    await flushPromises()
    expect(settings.data.value).toBe(true)
  })
})
