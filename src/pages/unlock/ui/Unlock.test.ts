import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'
import { useVaultStore } from '@/shared/store/vault'

import Unlock from './Unlock.vue'

vi.mock('vue-router', () => ({ useRouter: () => ({ replace: vi.fn(async () => undefined) }) }))

const mounted: ReturnType<typeof mount>[] = []

beforeEach(() => setActivePinia(createPinia()))

afterEach(() => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
  document.body.innerHTML = ''
})

describe('Unlock', () => {
  it('shows no title (no welcome flash) until the recents are loaded', async () => {
    let answer: (list: unknown[]) => void = () => undefined
    vi.mocked(call).mockImplementation((command: string) => {
      if (command === 'recent_vaults') return new Promise((resolve) => (answer = resolve)) as never
      return Promise.resolve(null) as never
    })
    const wrapper = mount(Unlock, { global: { stubs: { RecentVaults: true, VaultCreateDialog: true } } })
    mounted.push(wrapper)
    await flushPromises()
    expect(wrapper.find('[data-unlock-title]').exists()).toBe(false)
    const loading = useVaultStore().loadRecents()
    await flushPromises()
    expect(wrapper.find('[data-unlock-title]').exists()).toBe(false)
    expect(wrapper.find('[data-unlock-skeleton]').exists()).toBe(true)
    answer([])
    await loading
    await flushPromises()
    expect(wrapper.get('[data-unlock-title]').text()).toContain('Добро пожаловать')
  })
})

describe('Unlock after a failed read', () => {
  it('does not show the welcome when recent_vaults rejects', async () => {
    vi.mocked(call).mockImplementation((command: string) =>
      command === 'recent_vaults' ? (Promise.reject(new Error('boom')) as never) : (Promise.resolve(null) as never),
    )
    const wrapper = mount(Unlock, { global: { stubs: { RecentVaults: true, VaultCreateDialog: true } } })
    mounted.push(wrapper)
    await expect(useVaultStore().loadRecents()).rejects.toThrow('boom')
    await flushPromises()
    expect(useVaultStore().recentsLoaded).toBe(false)
    expect(useVaultStore().recentsError).toBe('boom')
    expect(wrapper.find('[data-unlock-title]').exists()).toBe(false)
  })
})
