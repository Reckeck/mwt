import { flushPromises, mount } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia } from 'pinia'
import { computed, ref, type Plugin } from 'vue'
import { describe, expect, it, vi } from 'vitest'

import SyncDevices from './SyncDevices.vue'
import SyncPendingChanges from './SyncPendingChanges.vue'

const statusData = ref<unknown>(null)
const sessionList = ref<unknown[]>([])

vi.mock('@/features/vault-sync', async (importOriginal) => {
  const mutation = { mutate: vi.fn(), isPending: ref(false) }
  return {
    ...(await importOriginal<typeof import('@/features/vault-sync')>()),
    useSyncStatus: () => ({
      status: { data: statusData, isPending: ref(false) },
      offers: computed(() => []),
      sessions: computed(() => sessionList.value),
    }),
    useSyncPeers: () => ({
      peers: { isPending: ref(false), error: ref(null), refetch: vi.fn() },
      list: computed(() => []),
      forgotten: computed(() => []),
    }),
    useSyncActions: () => ({
      pair: mutation,
      pairAgain: mutation,
      forget: mutation,
      resume: mutation,
      decidePlan: mutation,
    }),
    useSyncAgo: () => () => '',
  }
})

const plugins: (Plugin | [Plugin, ...unknown[]])[] = [
  createPinia(),
  [VueQueryPlugin, { queryClient: new QueryClient() }],
]

const count = (haystack: string, needle: string) => haystack.split(needle).length - 1

describe('SyncDevices main marker', () => {
  const mountWith = async (status: object) => {
    statusData.value = { enabled: true, device_name: 'Laptop', ...status }
    const wrapper = mount(SyncDevices, { global: { plugins } })
    await flushPromises()
    return wrapper
  }

  it('does not show prefer_main as the role', async () => {
    const wrapper = await mountWith({ role: 'client', prefer_main: true })
    expect(count(wrapper.text(), 'главное')).toBe(0)
  })

  it('shows Main once when the role is main', async () => {
    const wrapper = await mountWith({ role: 'main', prefer_main: false })
    expect(count(wrapper.text(), 'главное')).toBe(1)
  })
})

describe('SyncPendingChanges', () => {
  const item = (label: string) => ({ kind: 'added', label })
  const session = (id: string, total: number, items: number) => ({
    session_id: id,
    name: id,
    phase: 'awaiting_confirm',
    plan: {
      summary: {
        total,
        collections: [
          {
            collection: 'transactions',
            added: total,
            updated: 0,
            deleted: 0,
            items: Array.from({ length: items }, (_, index) => item(`row ${index}`)),
          },
        ],
        overwritten: [],
        unstamped_conflicts: 0,
      },
    },
  })

  it('counts and cuts each session on its own, with the tour marker once', async () => {
    sessionList.value = [session('Phone', 10, 4), session('Tablet', 3, 3)]
    const wrapper = mount(SyncPendingChanges, { global: { plugins } })
    await flushPromises()
    const text = wrapper.text()
    expect(text).toContain('Ждут подтверждения · 10')
    expect(text).toContain('Ждут подтверждения · 3')
    // 10 changes, 4 rows listed (the backend cut the list): six more, not four.
    expect(text).toContain('и ещё 6')
    expect(wrapper.findAll('[data-tour="sync-pending"]')).toHaveLength(1)
  })
})
