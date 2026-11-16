import { VueQueryPlugin, QueryClient } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'

import CryptoHiddenTokens from './CryptoHiddenTokens.vue'

const hidden: TokenView = {
  chain: 'ethereum',
  contract: '0x1111111111111111111111111111111111111111',
  symbol: 'SPAM',
  status: 'rejected',
  decided_at: '2026-10-01T10:00:00Z',
} as TokenView

const issued = () => vi.mocked(call).mock.calls.map(([cmd]) => cmd)

async function mountHidden() {
  vi.mocked(call).mockImplementation(async (cmd: string) => {
    if (cmd === 'crypto_list_tokens') return [hidden]
    if (cmd === 'crypto_list_accounts') return []
    if (cmd === 'crypto_network_settings') return { networks: [] }
    return null
  })
  const queryClient = new QueryClient()
  const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
  const wrapper = mount(CryptoHiddenTokens, {
    global: { plugins: [[VueQueryPlugin, { queryClient }]], stubs: { RouterLink: true } },
  })
  await flushPromises()
  return { wrapper, queryClient, invalidate }
}

describe('CryptoHiddenTokens', () => {
  beforeEach(() => vi.mocked(call).mockReset())

  it('returns a hidden token to the quarantine, never straight into the capital', async () => {
    const { wrapper } = await mountHidden()
    await wrapper.find('li button').trigger('click')
    await flushPromises()
    expect(issued()).toContain('crypto_clear_token_verdict')
    expect(issued()).not.toContain('crypto_confirm_token')
  })

  it('refreshes the wallet totals and the ledger figures, not only the token list', async () => {
    const { wrapper, invalidate } = await mountHidden()
    await wrapper.find('li button').trigger('click')
    await flushPromises()
    const keys = invalidate.mock.calls.map(([filters]) => JSON.stringify((filters as { queryKey: unknown }).queryKey))
    expect(keys).toContain(JSON.stringify(['crypto', 'accounts']))
    expect(keys).toContain(JSON.stringify(['crypto', 'tokens']))
    expect(keys.length).toBeGreaterThan(2)
  })

  it('undoes by hiding the token again', async () => {
    const { wrapper, queryClient } = await mountHidden()
    await wrapper.find('li button').trigger('click')
    await flushPromises()
    vi.mocked(call).mockClear()
    const meta = queryClient.getMutationCache().getAll()[0].options.meta as {
      undo: (data: unknown, token: unknown) => Promise<void>
    }
    await meta.undo(null, hidden)
    expect(issued()[0]).toBe('crypto_reject_token')
    expect(issued()).not.toContain('crypto_confirm_token')
  })
})
