import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'

import RecentVaults from './RecentVaults.vue'

const mounted: ReturnType<typeof mount>[] = []

beforeEach(() => setActivePinia(createPinia()))

afterEach(() => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
  document.body.innerHTML = ''
})

describe('RecentVaults after a failed read', () => {
  it('keeps the error, Retry and the open-file button; Retry reads again', async () => {
    vi.mocked(call).mockImplementation((command: string) =>
      command === 'recent_vaults' ? (Promise.reject(new Error('boom')) as never) : (Promise.resolve(null) as never),
    )
    const wrapper = mount(RecentVaults, {
      global: {
        plugins: [VueQueryPlugin],
        stubs: { VaultUnlockDialog: true, VaultDeleteDialog: true, VaultUnlockForm: true },
      },
    })
    mounted.push(wrapper)
    await flushPromises()
    expect(wrapper.find('[data-recents-error]').exists()).toBe(true)
    expect(wrapper.find('[data-recents-retry]').exists()).toBe(true)
    expect(wrapper.find('button:not([data-recents-retry]):not([data-unlock-create])').exists()).toBe(true)

    const reads = () => vi.mocked(call).mock.calls.filter(([command]) => command === 'recent_vaults').length
    const before = reads()
    await wrapper.get('[data-recents-retry]').trigger('click')
    await flushPromises()
    expect(reads()).toBe(before + 1)
  })
})
