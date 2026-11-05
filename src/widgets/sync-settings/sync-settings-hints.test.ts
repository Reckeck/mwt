import { flushPromises, mount } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia } from 'pinia'
import { computed, ref, type Plugin } from 'vue'
import { describe, expect, it, vi } from 'vitest'

import SyncNetwork from './SyncNetwork.vue'
import SyncSettings from './SyncSettings.vue'

const statusData = ref<unknown>(null)
const openSettings = vi.fn()

vi.mock('@/features/vault-sync', async (importOriginal) => {
  const mutation = { mutate: vi.fn(), isPending: ref(false) }
  return {
    ...(await importOriginal<typeof import('@/features/vault-sync')>()),
    useSyncStatus: () => ({
      status: { data: statusData, isPending: ref(false), error: ref(null), refetch: vi.fn() },
      connected: computed(() => []),
    }),
    useSyncActions: () => ({
      settings: mutation,
      restore: mutation,
      openFirewall: mutation,
      copyCommand: mutation,
      openNotificationSettings: { mutate: openSettings, isPending: ref(false) },
    }),
  }
})

const plugins: (Plugin | [Plugin, ...unknown[]])[] = [
  createPinia(),
  [VueQueryPlugin, { queryClient: new QueryClient() }],
]

const network = (patch: object) => ({
  port: 53318,
  address: '192.168.1.5',
  seen: 0,
  since_ms: Date.now(),
  inbound_blocked: true,
  network_public: false,
  firewall_fix: 'auto',
  firewall_commands: ['netsh advfirewall firewall add rule'],
  last_error: null,
  ...patch,
})

describe('notification hint in the sync settings', () => {
  const mountWith = async (patch: object) => {
    statusData.value = {
      enabled: true,
      device_name: 'Phone',
      notifications_blocked: false,
      network: network({ inbound_blocked: false }),
      ...patch,
    }
    // The status line and the network block have their own coverage.
    const wrapper = mount(SyncSettings, { global: { plugins, stubs: { SyncStatusLine: true, SyncNetwork: true } } })
    await flushPromises()
    return wrapper
  }

  it('shows the hint with the settings action when notifications are blocked', async () => {
    const wrapper = await mountWith({ notifications_blocked: true })
    const hint = wrapper.find('[data-sync-notifications]')
    expect(hint.exists()).toBe(true)
    expect(hint.text()).toContain('Уведомления выключены')
    await hint.find('button').trigger('click')
    expect(openSettings).toHaveBeenCalledTimes(1)
  })

  it('hides the hint when notifications are allowed', async () => {
    const wrapper = await mountWith({ notifications_blocked: false })
    expect(wrapper.find('[data-sync-notifications]').exists()).toBe(false)
  })

  it('hides the hint when sync is off', async () => {
    const wrapper = await mountWith({ enabled: false, notifications_blocked: true })
    expect(wrapper.find('[data-sync-notifications]').exists()).toBe(false)
  })
})

describe('Public network text in the firewall banner', () => {
  const mountWith = async (patch: object) => {
    statusData.value = { enabled: true, network: network(patch) }
    const wrapper = mount(SyncNetwork, { global: { plugins } })
    await flushPromises()
    return wrapper
  }

  it('shows only with the banner and the Public flag together', async () => {
    const both = await mountWith({ inbound_blocked: true, network_public: true })
    expect(both.find('[data-sync-public-network]').exists()).toBe(true)
    expect(both.text()).toContain('общедоступной')
  })

  it('is hidden when the network is not Public', async () => {
    const wrapper = await mountWith({ inbound_blocked: true, network_public: false })
    expect(wrapper.find('[data-sync-public-network]').exists()).toBe(false)
  })

  it('is hidden when nothing is blocked', async () => {
    const wrapper = await mountWith({ inbound_blocked: false, network_public: true })
    expect(wrapper.find('[data-sync-public-network]').exists()).toBe(false)
  })
})
